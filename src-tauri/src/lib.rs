use tauri::{Manager, Emitter};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::WindowEvent;
use std::time::Duration;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn hide_window(window: tauri::Window) {
    let _ = window.emit("animate-hide", ());
    // Wait for the CSS animation to complete (0.3s)
    std::thread::sleep(Duration::from_millis(300));
    let _ = window.hide();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let main_window = app.get_webview_window("main").expect("main window not found");

            // Initial positioning
            if let Some(monitor) = main_window.current_monitor()? {
                let monitor_size = monitor.size();
                let window_size = main_window.inner_size()?;
                let work_area = monitor.work_area();
                let taskbar_height = monitor_size.height as f64 - work_area.size.height as f64;
                let x = monitor_size.width as f64 - window_size.width as f64 - 24.0;
                let y = monitor_size.height as f64 - window_size.height as f64 - taskbar_height - 18.0;
                main_window.set_position(tauri::LogicalPosition::new(x, y))?;
                main_window.show()?;
                main_window.set_focus()?;
            }

            // Tray icon setup
            let icon = app.default_window_icon().unwrap().clone();
            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.emit("animate-show", ());
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Trigger animation on blur, then hide
            let window_clone = main_window.clone();
            main_window.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    let w = window_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = w.emit("animate-hide", ());
                        std::thread::sleep(Duration::from_millis(300));
                        let _ = w.hide();
                    });
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, hide_window])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
