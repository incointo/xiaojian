use crate::tray_helpers::popup_position;
use std::sync::Mutex;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};

pub const TRAY_ID: &str = "xiaojian-tray";
static QUICK_OPEN_LOCK: Mutex<()> = Mutex::new(());

pub fn build_tray(app: &AppHandle) -> tauri::Result<tauri::tray::TrayIcon> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::include_image!("icons/32x32.png"))
        .tooltip("小笺 · 左键打开便笺，右键查看清单")
        .show_menu_on_left_click(false)
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
                        // WebView2 creation must not block the Windows event loop.
                        let app = app.clone();
                        std::thread::spawn(move || {
                            if let Err(error) = show_quick_window(&app, position) {
                                eprintln!("Failed to open quick checklist: {error}");
                                crate::show_window_and_refresh(&app);
                            }
                        });
                    }
                    _ => {}
                }
            }
        })
        .build(app)
}

fn show_quick_window(app: &AppHandle, pointer: PhysicalPosition<f64>) -> tauri::Result<()> {
    // Rapid repeated tray clicks must not race to create the same window label.
    let _opening = QUICK_OPEN_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let window = match app.get_webview_window("quick") {
        Some(window) => window,
        None => {
            let window =
                WebviewWindowBuilder::new(app, "quick", WebviewUrl::App("quick.html".into()))
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
            let dismiss = window.clone();
            window.on_window_event(move |event| match event {
                WindowEvent::Focused(false) => {
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
    window.set_focus()?;
    window.emit("quick-opened", ())?;
    Ok(())
}

pub fn hide_quick_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("quick") {
        let _ = window.hide();
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
