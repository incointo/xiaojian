use crate::tasks::{is_complete_today, Task, TaskKind};
use crate::tray_helpers::{menu_item_id, menu_item_title};
use chrono::{Local, NaiveDate};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

pub const TRAY_ID: &str = "xiaojian-tray";

pub fn build_menu<R: Runtime, M: Manager<R>>(
    manager: &M,
    tasks: &[Task],
) -> tauri::Result<Menu<R>> {
    let today = Local::now().date_naive();
    let menu = Menu::new(manager)?;

    menu.append(&MenuItem::with_id(
        manager,
        "open-window",
        "打开小笺",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(manager)?)?;

    menu.append(&MenuItem::new(manager, "今日", false, None::<&str>)?)?;
    append_task_group(manager, &menu, tasks, TaskKind::Today, today)?;

    menu.append(&PredefinedMenuItem::separator(manager)?)?;
    menu.append(&MenuItem::new(manager, "每日", false, None::<&str>)?)?;
    append_task_group(manager, &menu, tasks, TaskKind::Daily, today)?;

    menu.append(&PredefinedMenuItem::separator(manager)?)?;
    menu.append(&MenuItem::with_id(
        manager,
        "exit-app",
        "退出",
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

pub fn refresh_menu<R: Runtime>(app: &AppHandle<R>, tasks: &[Task]) -> Result<(), String> {
    let menu = build_menu(app, tasks).map_err(|error| error.to_string())?;
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "system tray icon is unavailable".to_owned())?;
    tray.set_menu(Some(menu)).map_err(|error| error.to_string())
}

pub fn build_tray(
    app: &AppHandle,
    menu: &Menu<tauri::Wry>,
) -> tauri::Result<tauri::tray::TrayIcon> {
    let icon = tauri::include_image!("icons/32x32.png");
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("小笺")
        .menu(menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                show_main_window(app);
            }
        })
        .on_menu_event(|app, event| crate::handle_tray_menu_event(app, event.id().as_ref()))
        .build(app)
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn append_task_group<R: Runtime, M: Manager<R>>(
    manager: &M,
    menu: &Menu<R>,
    tasks: &[Task],
    kind: TaskKind,
    today: NaiveDate,
) -> tauri::Result<()> {
    let mut found = false;
    for task in tasks
        .iter()
        .filter(|task| task.kind == kind && is_active_in_tray(task, today))
    {
        let label = menu_item_title(&task.title).replace('&', "&&");
        let item = MenuItem::with_id(manager, menu_item_id(task.id), label, true, None::<&str>)?;
        menu.append(&item)?;
        found = true;
    }
    if !found {
        menu.append(&MenuItem::new(manager, "没有待办", false, None::<&str>)?)?;
    }
    Ok(())
}

fn is_active_in_tray(task: &Task, today: NaiveDate) -> bool {
    match task.kind {
        TaskKind::Today => task.due_date == Some(today) && !is_complete_today(task, today),
        TaskKind::Daily => !is_complete_today(task, today),
    }
}
