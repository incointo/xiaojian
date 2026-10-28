use crate::tray_helpers::popup_position;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

pub const TRAY_ID: &str = "xiaojian-tray";
static QUICK_OPEN_LOCK: Mutex<()> = Mutex::new(());
static QUICK_LABEL: Mutex<Option<String>> = Mutex::new(None);
static NEXT_QUICK_ID: AtomicU64 = AtomicU64::new(1);

pub fn build_tray(app: &AppHandle) -> tauri::Result<tauri::tray::TrayIcon> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::include_image!("icons/32x32.png"))
        .tooltip("小笺 · 左键打开便笺，右键查看清单")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            match id {
                "fallback-open" => crate::show_window_and_refresh(app),
                "fallback-exit" => crate::request_exit(app),
                _ => {
                    if let Some(value) = id.strip_prefix("fallback-task:") {
                        if let Some((id, completed)) = value.split_once(':') {
                            if let Ok(id) = id.parse::<i64>() {
                                let result = crate::mutate_tasks(
                                    app,
                                    &app.state::<crate::AppState>(),
                                    |store, today| {
                                        store.set_task_completed(id, completed == "true", today)
                                    },
                                );
                                if let Err(error) = result {
                                    trace(app, "native checklist task save failed");
                                    crate::show_window_and_refresh(app);
                                    let _ = app.emit("quick-open-error", error);
                                }
                            }
                        }
                    }
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                let app = tray.app_handle();
                match button {
                    MouseButton::Left => crate::show_window_and_refresh(app),
                    MouseButton::Right => {
                        trace(app, "tray right-click received");
                        open_quick_checklist(app, position);
                    }
                    _ => {}
                }
            }
        })
        .build(app)
}

pub fn trace(app: &AppHandle, message: &str) {
    if let Ok(directory) = app.path().app_log_dir() {
        if std::fs::create_dir_all(&directory).is_ok() {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(directory.join("window-events.log"))
            {
                if file
                    .metadata()
                    .map(|metadata| metadata.len() > 256 * 1024)
                    .unwrap_or(false)
                {
                    let _ = file.set_len(0);
                }
                let _ = writeln!(
                    file,
                    "{} {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                    message
                );
            }
        }
    }
}

pub fn open_quick_checklist(app: &AppHandle, pointer: PhysicalPosition<f64>) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(error) = show_quick_window(&app, pointer) {
            trace(&app, &format!("quick open failed: {error}"));
            if let Err(fallback_error) = show_native_checklist(&app) {
                trace(&app, &format!("native checklist failed: {fallback_error}"));
                crate::show_window_and_refresh(&app);
                let _ = app.emit(
                    "quick-open-error",
                    "快捷清单暂时无法打开，请在主窗口操作任务。",
                );
            }
        }
    });
}

fn show_quick_window(app: &AppHandle, pointer: PhysicalPosition<f64>) -> tauri::Result<()> {
    trace(app, "quick open requested");
    // Rapid repeated tray clicks must not race to create the same window label.
    let _opening = QUICK_OPEN_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let label = QUICK_LABEL
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let existing = label
        .as_deref()
        .and_then(|label| app.get_webview_window(label));
    let result = (|| {
        let window = match existing {
            Some(window) => window,
            None => {
                // Windows shares a WebView2 browser process between these windows.
                // Its environment options must match the main window exactly.
                let browser_args = app
                    .config()
                    .app
                    .windows
                    .iter()
                    .find(|config| config.label == "main")
                    .and_then(|config| config.additional_browser_args.as_deref())
                    .unwrap_or_default();
                // A failed asynchronous WebView creation can leave a registered window.
                // Use a fresh label for recovery instead of reopening that dead handle.
                let label = format!("quick-{}", NEXT_QUICK_ID.fetch_add(1, Ordering::Relaxed));
                *QUICK_LABEL
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(label.clone());
                let window =
                    WebviewWindowBuilder::new(app, &label, WebviewUrl::App("quick.html".into()))
                        .additional_browser_args(browser_args)
                        .title("小笺 · 快捷清单")
                        .inner_size(340.0, 432.0)
                        .decorations(false)
                        .transparent(true)
                        .shadow(false)
                        .resizable(false)
                        .skip_taskbar(true)
                        .always_on_top(true)
                        .visible(false)
                        .focused(false)
                        .build()?;
                // build() may return before the underlying WebView initialization failed.
                window.is_visible()?;
                trace(app, "quick window ready");
                let dismiss = window.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::Focused(false) => {
                        trace(dismiss.app_handle(), "quick focus lost, hiding");
                        let _ = dismiss.hide();
                    }
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = dismiss.hide();
                    }
                    _ => {}
                });
                window
            }
        };

        if let Some(monitor) = window
            .monitor_from_point(pointer.x, pointer.y)?
            .or(app.primary_monitor()?)
        {
            let area = monitor.work_area();
            let scale = monitor.scale_factor();
            let size = PhysicalSize::new(
                ((340.0 * scale) as u32).min(area.size.width.saturating_sub(24).max(1)),
                ((432.0 * scale) as u32).min(area.size.height.saturating_sub(24).max(1)),
            );
            let (x, y) = popup_position(
                (pointer.x as i32, pointer.y as i32),
                (size.width, size.height),
                (
                    area.position.x,
                    area.position.y,
                    area.size.width,
                    area.size.height,
                ),
            );
            window.set_position(PhysicalPosition::new(x, y))?;
            window.set_size(size)?;
        }
        window.show()?;
        trace(app, "quick show requested");
        window.set_focus()?;
        trace(
            app,
            &format!(
                "quick focus requested; visible={:?}, focused={:?}",
                window.is_visible(),
                window.is_focused()
            ),
        );
        window.emit("quick-opened", ())?;
        let _ = app.emit("quick-open-recovered", ());
        Ok(())
    })();
    if result.is_err() {
        if let Some(window) = quick_window(app) {
            let _ = window.destroy();
        }
        *QUICK_LABEL
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
    }
    result
}

fn quick_window(app: &AppHandle) -> Option<WebviewWindow> {
    QUICK_LABEL
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_deref()
        .and_then(|label| app.get_webview_window(label))
}

pub fn hide_quick_window(app: &AppHandle) {
    if let Some(window) = quick_window(app) {
        let _ = window.hide();
    }
}

fn show_native_checklist(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "fallback-open",
        "打开小笺",
        true,
        None::<&str>,
    )?)?;
    let tasks = crate::list_tasks_from_store(&app.state::<crate::AppState>());
    match tasks {
        Ok(tasks) => {
            let today = chrono::Local::now().date_naive();
            for (kind, title) in [
                (crate::tasks::TaskKind::Today, "今日"),
                (crate::tasks::TaskKind::Daily, "每日"),
            ] {
                menu.append(&PredefinedMenuItem::separator(app)?)?;
                menu.append(&MenuItem::new(app, title, false, None::<&str>)?)?;
                let group: Vec<_> = tasks.iter().filter(|task| task.kind == kind).collect();
                if group.is_empty() {
                    menu.append(&MenuItem::new(app, "没有待办", false, None::<&str>)?)?;
                }
                for task in group {
                    let completed = if kind == crate::tasks::TaskKind::Daily {
                        task.completed_on == Some(today)
                    } else {
                        task.completed_on.is_some()
                    };
                    let title = task.title.replace(['\r', '\n', '\t'], " ");
                    let mut text: String = title.chars().take(60).collect();
                    if title.chars().count() > 60 {
                        text.push('…');
                    }
                    menu.append(&CheckMenuItem::with_id(
                        app,
                        format!("fallback-task:{}:{}", task.id, !completed),
                        text.replace('&', "&&"),
                        true,
                        completed,
                        None::<&str>,
                    )?)?;
                }
            }
        }
        Err(_) => menu.append(&MenuItem::new(
            app,
            "任务读取失败，请打开主窗口查看",
            false,
            None::<&str>,
        )?)?,
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "fallback-exit",
        "退出小笺",
        true,
        None::<&str>,
    )?)?;
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    trace(app, "native checklist opened");
    window.popup_menu(&menu)
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
