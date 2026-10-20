pub mod storage;
pub mod tasks;
pub mod tray;
pub mod tray_helpers;

use chrono::{Local, NaiveDate};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;
use storage::{Store, StoreError};
use tasks::{is_complete_today, Task, TaskKind};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};

pub struct AppState {
    store: Mutex<Option<Store>>,
    startup_error: Option<String>,
}

fn store_lock(state: &AppState) -> Result<MutexGuard<'_, Option<Store>>, String> {
    let guard = state
        .store
        .lock()
        .map_err(|_| "本地数据锁定失败，请重启小笺。".to_owned())?;
    if guard.is_none() {
        return Err(state
            .startup_error
            .clone()
            .unwrap_or_else(|| "本地数据库当前不可用。".to_owned()));
    }
    Ok(guard)
}

fn available_store_mut<'a>(
    guard: &'a mut MutexGuard<'_, Option<Store>>,
) -> Result<&'a mut Store, String> {
    guard
        .as_mut()
        .ok_or_else(|| "本地数据库当前不可用。".to_owned())
}

fn available_store_ref<'a>(guard: &'a MutexGuard<'_, Option<Store>>) -> Result<&'a Store, String> {
    guard
        .as_ref()
        .ok_or_else(|| "本地数据库当前不可用。".to_owned())
}

fn list_tasks_from_store(state: &AppState) -> Result<Vec<Task>, String> {
    let today = Local::now().date_naive();
    let mut guard = store_lock(state)?;
    available_store_mut(&mut guard)?
        .list_tasks(today)
        .map_err(|error| error.to_string())
}

fn mutate_tasks<T>(
    app: &AppHandle,
    state: &AppState,
    mutation: impl FnOnce(&mut Store, NaiveDate) -> Result<T, StoreError>,
) -> Result<T, String> {
    let today = Local::now().date_naive();
    let (value, tasks) = {
        let mut guard = store_lock(state)?;
        let store = available_store_mut(&mut guard)?;
        let value = mutation(store, today).map_err(|error| error.to_string())?;
        let tasks = store.list_tasks(today);
        (value, tasks)
    };

    match tasks {
        Ok(tasks) => {
            if let Err(error) = tray::refresh_menu(app, &tasks) {
                eprintln!("Task was saved, but the tray menu could not refresh: {error}");
            }
        }
        Err(error) => eprintln!("Task was saved, but the task list could not refresh: {error}"),
    }
    Ok(value)
}

#[tauri::command]
fn list_tasks(state: State<'_, AppState>, app: AppHandle) -> Result<Vec<Task>, String> {
    let tasks = list_tasks_from_store(&state)?;
    if let Err(error) = tray::refresh_menu(&app, &tasks) {
        eprintln!("Task list loaded, but the tray menu could not refresh: {error}");
    }
    Ok(tasks)
}

#[tauri::command]
fn create_task(
    title: String,
    kind: TaskKind,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Task, String> {
    mutate_tasks(&app, &state, |store, today| {
        store.create_task(&title, kind, today)
    })
}

#[tauri::command]
fn set_task_completed(
    id: i64,
    completed: bool,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    mutate_tasks(&app, &state, |store, today| {
        store.set_task_completed(id, completed, today)
    })
}

#[tauri::command]
fn rename_task(
    id: i64,
    title: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    mutate_tasks(&app, &state, |store, _today| store.rename_task(id, &title))
}

#[tauri::command]
fn delete_task(id: i64, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    mutate_tasks(&app, &state, |store, _today| store.delete_task(id))
}

#[tauri::command]
fn load_note(state: State<'_, AppState>) -> Result<String, String> {
    let guard = store_lock(&state)?;
    available_store_ref(&guard)?
        .load_note()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_note(note: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = store_lock(&state)?;
    available_store_mut(&mut guard)?
        .save_note(&note)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn hide_window(window: WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn exit_after_save(app: AppHandle) {
    app.exit(0);
}

fn app_data_database(app: &tauri::App) -> Result<(PathBuf, bool), String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法取得小笺数据目录：{error}"))?;
    let path = directory.join("xiaojian.sqlite3");
    let first_launch = !path.exists();
    fs::create_dir_all(&directory)
        .map_err(|error| format!("无法创建小笺数据目录 {}：{error}", directory.display()))?;
    Ok((path, first_launch))
}

fn initialize_store(app: &tauri::App) -> (Option<Store>, Option<String>, Vec<Task>, bool) {
    let (path, first_launch) = match app_data_database(app) {
        Ok(result) => result,
        Err(error) => return (None, Some(error), Vec::new(), false),
    };
    let mut store = match Store::open(&path) {
        Ok(store) => store,
        Err(error) => {
            return (
                None,
                Some(format!("无法打开本地数据库 {}：{error}", path.display())),
                Vec::new(),
                first_launch,
            );
        }
    };
    let tasks = match store.list_tasks(Local::now().date_naive()) {
        Ok(tasks) => tasks,
        Err(error) => {
            return (
                None,
                Some(format!("无法读取本地数据库 {}：{error}", path.display())),
                Vec::new(),
                first_launch,
            );
        }
    };
    (Some(store), None, tasks, first_launch)
}

fn place_window_bottom_right(app: &AppHandle, window: &WebviewWindow) -> Result<(), tauri::Error> {
    let Some(monitor) = app.primary_monitor()? else {
        return Ok(());
    };
    let work_area = monitor.work_area();
    let window_size = window.outer_size()?;
    let margin: i32 = 12;
    let min_x = work_area.position.x;
    let min_y = work_area.position.y;
    let max_x = work_area.position.x + work_area.size.width as i32 - window_size.width as i32;
    let max_y = work_area.position.y + work_area.size.height as i32 - window_size.height as i32;
    let x =
        (work_area.position.x + work_area.size.width as i32 - window_size.width as i32 - margin)
            .clamp(min_x, max_x.max(min_x));
    let y =
        (work_area.position.y + work_area.size.height as i32 - window_size.height as i32 - margin)
            .clamp(min_y, max_y.max(min_y));
    window.set_position(tauri::PhysicalPosition::new(x, y))?;
    Ok(())
}

fn active_tray_task(task: &Task, today: NaiveDate) -> bool {
    match task.kind {
        TaskKind::Today => task.due_date == Some(today) && !is_complete_today(task, today),
        TaskKind::Daily => !is_complete_today(task, today),
    }
}

fn emit_tasks_changed(app: &AppHandle) {
    let _ = app.emit("tasks-changed", ());
}

fn refresh_tasks_and_menu(app: &AppHandle) -> Result<Vec<Task>, String> {
    let state = app.state::<AppState>();
    let tasks = list_tasks_from_store(&state)?;
    tray::refresh_menu(app, &tasks)?;
    Ok(tasks)
}

fn show_window_and_refresh(app: &AppHandle) {
    if refresh_tasks_and_menu(app).is_ok() {
        emit_tasks_changed(app);
    }
    tray::show_main_window(app);
}

fn request_exit(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        app.exit(0);
        return;
    };
    let _ = window.show();
    let _ = window.set_focus();
    if let Err(error) = window.emit("quit-requested", ()) {
        eprintln!("Failed to request a save before exit: {error}");
    }
}

fn complete_task_from_tray(app: &AppHandle, task_id: i64) {
    let today = Local::now().date_naive();
    let result = (|| {
        let state = app.state::<AppState>();
        let mut guard = store_lock(&state)?;
        let store = available_store_mut(&mut guard)?;
        let tasks = store.list_tasks(today).map_err(|error| error.to_string())?;
        if let Some(task) = tasks.iter().find(|task| task.id == task_id) {
            if active_tray_task(task, today) {
                store
                    .set_task_completed(task_id, true, today)
                    .map_err(|error| error.to_string())?;
            }
        }
        store.list_tasks(today).map_err(|error| error.to_string())
    })();

    match result {
        Ok(tasks) => {
            if let Err(error) = tray::refresh_menu(app, &tasks) {
                eprintln!("Failed to refresh tray menu: {error}");
            }
            emit_tasks_changed(app);
        }
        Err(error) => eprintln!("Failed to complete tray task: {error}"),
    }
}

pub(crate) fn handle_tray_menu_event(app: &AppHandle, event: &str) {
    match event {
        "open-window" => show_window_and_refresh(app),
        "exit-app" => request_exit(app),
        _ => {
            if let Some(id) = event.strip_prefix("complete-") {
                if let Ok(task_id) = id.parse::<i64>() {
                    complete_task_from_tray(app, task_id);
                }
            }
        }
    }
}

fn start_date_monitor(app: AppHandle) {
    thread::spawn(move || {
        let mut last_date = Local::now().date_naive();
        loop {
            thread::sleep(Duration::from_secs(60));
            let current_date = Local::now().date_naive();
            if current_date != last_date {
                last_date = current_date;
                match refresh_tasks_and_menu(&app) {
                    Ok(_) => emit_tasks_changed(&app),
                    Err(error) => eprintln!("Failed to refresh tasks after date change: {error}"),
                }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_window_and_refresh(app);
        }))
        .setup(|app| {
            let (store, startup_error, tasks, first_launch) = initialize_store(app);
            app.manage(AppState {
                store: Mutex::new(store),
                startup_error,
            });

            let main_window = app
                .get_webview_window("main")
                .expect("main window not found");
            let app_handle = app.handle().clone();
            if first_launch {
                place_window_bottom_right(&app_handle, &main_window)?;
            }

            let menu = tray::build_menu(app, &tasks)?;
            tray::build_tray(&app_handle, &menu)?;

            let window_for_events = main_window.clone();
            main_window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window_for_events.hide();
                }
            });

            start_date_monitor(app_handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_tasks,
            create_task,
            set_task_completed,
            rename_task,
            delete_task,
            load_note,
            save_note,
            hide_window,
            exit_after_save
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
