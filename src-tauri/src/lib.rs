use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    title: String,
}

fn tasks_file_path() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("Failed to get exe path: {e}"))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "Failed to get executable directory".to_string())?;
    Ok(exe_dir.join("tasks.json"))
}

fn write_tasks_file(path: &PathBuf, tasks: &[Task]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(tasks)
        .map_err(|e| format!("Failed to serialize tasks: {e}"))?;
    fs::write(path, json).map_err(|e| format!("Failed to write tasks file: {e}"))
}

#[tauri::command]
fn load_tasks() -> Result<Vec<Task>, String> {
    let path = tasks_file_path()?;

    if !path.exists() {
        let tasks: Vec<Task> = vec![];
        write_tasks_file(&path, &tasks)?;
        return Ok(tasks);
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read tasks file at {}: {e}", path.display()))?;

    if content.trim().is_empty() {
        return Ok(vec![]);
    }

    let tasks: Vec<Task> = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid tasks.json format at {}: {e}", path.display()))?;

    Ok(tasks)
}

#[tauri::command]
fn save_tasks(tasks: Vec<Task>) -> Result<(), String> {
    let path = tasks_file_path()?;
    write_tasks_file(&path, &tasks)
}

#[tauri::command]
async fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let main_window = app.get_webview_window("main").expect("main window not found");

            // Initial positioning
            if let Some(monitor) = app.primary_monitor()? {
                let window_size = main_window.inner_size()?;
                let work_area = monitor.work_area();

                let x = work_area.position.x as f64 + work_area.size.width as f64
                    - window_size.width as f64
                    - 24.0;
                let y = work_area.position.y as f64 + work_area.size.height as f64
                    - window_size.height as f64
                    - 18.0;

                main_window.set_position(tauri::LogicalPosition::new(x, y))?;
            }

            // Tray icon setup (use explicit asset for reliable updates)
            let icon = tauri::include_image!("icons/32x32.png");
            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Hide on blur after startup grace period
            let window_clone = main_window.clone();
            let startup_instant = Instant::now();
            main_window.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    if startup_instant.elapsed() < Duration::from_millis(900) {
                        return;
                    }
                    let _ = window_clone.hide();
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![hide_window, load_tasks, save_tasks])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
