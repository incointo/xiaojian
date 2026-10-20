pub mod storage;
pub mod tasks;
pub mod tray;
pub mod tray_helpers;
pub mod window_layout;

use chrono::{Local, NaiveDate};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;
use storage::{Store, StoreError};
use tasks::{Task, TaskKind};
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
    let value = {
        let mut guard = store_lock(state)?;
        let store = available_store_mut(&mut guard)?;
        mutation(store, today).map_err(|error| error.to_string())?
    };

    emit_tasks_changed(app);
    Ok(value)
}

#[tauri::command]
fn list_tasks(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    list_tasks_from_store(&state)
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
fn hide_window(window: WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn open_main_window(app: AppHandle) {
    show_window_and_refresh(&app);
}

#[tauri::command]
fn open_quick_checklist(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let size = window.outer_size().map_err(|error| error.to_string())?;
    tray::open_quick_checklist(
        &app,
        tauri::PhysicalPosition::new(
            (position.x + size.width as i32) as f64,
            (position.y + size.height as i32) as f64,
        ),
    );
    Ok(())
}

#[tauri::command]
fn request_app_exit(app: AppHandle) {
    request_exit(&app);
}

#[tauri::command]
fn exit_app(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // Mutations commit before releasing this lock, including writes from the quick window.
    let _store = state
        .store
        .lock()
        .map_err(|_| "任务保存状态不可用，请重试。".to_owned())?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn fit_main_height(window: WebviewWindow, height: f64) -> Result<(), String> {
    if window.label() != "main" {
        return Err("仅主窗口可自动调整高度。".to_owned());
    }
    fit_main_height_inner(&window, height).map_err(|error| error.to_string())
}

fn fit_main_height_inner(window: &WebviewWindow, height: f64) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()?.or(window.primary_monitor()?) else {
        return Ok(());
    };
    let area = monitor.work_area();
    let scale = window.scale_factor()?;
    let inner = window.inner_size()?;
    let outer = window.outer_size()?;
    let position = window.outer_position()?;
    let frame_height = outer.height.saturating_sub(inner.height);
    let Some(plan) = window_layout::main_height_plan(
        height,
        scale,
        area.position.y,
        area.size.height.saturating_sub(frame_height),
        position.y,
    ) else {
        return Ok(());
    };
    // Windows reapplies the current size when constraints change. Avoid repeatedly
    // setting identical constraints in response to our own resize notifications.
    static LAST_MINIMUM: Mutex<Option<(u32, u32)>> = Mutex::new(None);
    let minimum = (
        (window_layout::MIN_WIDTH * scale).ceil() as u32,
        plan.min_height,
    );
    {
        let mut previous = LAST_MINIMUM
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if *previous != Some(minimum) {
            window.set_min_size(Some(tauri::PhysicalSize::new(minimum.0, minimum.1)))?;
            *previous = Some(minimum);
        }
    }
    if inner.height.abs_diff(plan.height) > 1 {
        // Retain the current physical width; automatic fitting never chooses a width.
        window.set_size(tauri::PhysicalSize::new(inner.width, plan.height))?;
    }
    if position.y != plan.top {
        window.set_position(tauri::PhysicalPosition::new(position.x, plan.top))?;
    }
    Ok(())
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

fn emit_tasks_changed(app: &AppHandle) {
    let _ = app.emit("tasks-changed", ());
}

fn show_window_and_refresh(app: &AppHandle) {
    tray::hide_quick_window(app);
    tray::show_main_window(app);
    emit_tasks_changed(app);
}

fn request_exit(app: &AppHandle) {
    tray::hide_quick_window(app);
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

fn start_date_monitor(app: AppHandle) {
    thread::spawn(move || {
        let mut last_date = Local::now().date_naive();
        loop {
            thread::sleep(Duration::from_secs(60));
            let current_date = Local::now().date_naive();
            if current_date != last_date {
                last_date = current_date;
                emit_tasks_changed(&app);
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
            let (store, startup_error, _tasks, first_launch) = initialize_store(app);
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

            tray::build_tray(&app_handle)?;

            let window_for_events = main_window.clone();
            main_window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window_for_events.hide();
                }
                if matches!(
                    event,
                    WindowEvent::Moved(_)
                        | WindowEvent::Resized(_)
                        | WindowEvent::ScaleFactorChanged { .. }
                ) {
                    let _ = window_for_events.emit("window-geometry-changed", ());
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
            hide_window,
            open_main_window,
            open_quick_checklist,
            request_app_exit,
            exit_app,
            fit_main_height
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
