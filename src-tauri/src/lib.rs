pub mod storage;
pub mod tasks;

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};
#[cfg(target_os = "windows")]
use winreg::enums::HKEY_CURRENT_USER;
#[cfg(target_os = "windows")]
use winreg::RegKey;

const STARTUP_MENU_ID: &str = "launch-at-startup";
const STARTUP_REG_VALUE_NAME: &str = "TickNest";
#[cfg(target_os = "windows")]
const APP_PREFS_REG_PATH: &str = "Software\\TickNest";
#[cfg(target_os = "windows")]
const APP_PREF_STARTUP_VALUE_NAME: &str = "LaunchAtStartup";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    title: String,
}

fn tasks_file_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data directory: {e}"))?;

    fs::create_dir_all(&app_data_dir).map_err(|e| {
        format!(
            "Failed to create app data directory at {}: {e}",
            app_data_dir.display()
        )
    })?;

    Ok(app_data_dir.join("tasks.json"))
}

fn write_tasks_file(path: &PathBuf, tasks: &[Task]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(tasks)
        .map_err(|e| format!("Failed to serialize tasks: {e}"))?;

    let temp_path = path.with_extension("json.tmp");

    let mut temp_file = fs::File::create(&temp_path).map_err(|e| {
        format!(
            "Failed to create temp tasks file at {}: {e}",
            temp_path.display()
        )
    })?;
    temp_file.write_all(json.as_bytes()).map_err(|e| {
        format!(
            "Failed to write temp tasks file at {}: {e}",
            temp_path.display()
        )
    })?;
    temp_file.sync_all().map_err(|e| {
        format!(
            "Failed to flush temp tasks file at {}: {e}",
            temp_path.display()
        )
    })?;

    if path.exists() {
        let _ = fs::remove_file(path);
    }

    fs::rename(&temp_path, path).map_err(|e| {
        format!(
            "Failed to replace tasks file from {} to {}: {e}",
            temp_path.display(),
            path.display()
        )
    })
}

#[tauri::command]
fn load_tasks(app: tauri::AppHandle) -> Result<Vec<Task>, String> {
    let path = tasks_file_path(&app)?;

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
fn save_tasks(app: tauri::AppHandle, tasks: Vec<Task>) -> Result<(), String> {
    let path = tasks_file_path(&app)?;
    write_tasks_file(&path, &tasks)
}

#[tauri::command]
async fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

fn env_flag_enabled(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

fn place_window_bottom_right(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), tauri::Error> {
    let Some(monitor) = app.primary_monitor()? else {
        return Ok(());
    };

    let work_area = monitor.work_area();
    let window_size = window.outer_size()?;

    let margin_x: i32 = 12;
    let margin_y: i32 = 12;

    let min_x = work_area.position.x;
    let min_y = work_area.position.y;
    let max_x = work_area.position.x + work_area.size.width as i32 - window_size.width as i32;
    let max_y = work_area.position.y + work_area.size.height as i32 - window_size.height as i32;

    let desired_x =
        work_area.position.x + work_area.size.width as i32 - window_size.width as i32 - margin_x;
    let desired_y =
        work_area.position.y + work_area.size.height as i32 - window_size.height as i32 - margin_y;

    let clamped_x = desired_x.clamp(min_x, max_x.max(min_x));
    let clamped_y = desired_y.clamp(min_y, max_y.max(min_y));

    window.set_position(tauri::PhysicalPosition::new(clamped_x, clamped_y))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn startup_exe_command() -> Result<String, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("Failed to get exe path: {e}"))?;
    Ok(format!("\"{}\"", exe_path.display()))
}

#[cfg(target_os = "windows")]
fn windows_run_key() -> Result<RegKey, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _) = hkcu
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map_err(|e| format!("Failed to open startup registry key: {e}"))?;
    Ok(run_key)
}

#[cfg(target_os = "windows")]
fn windows_prefs_key() -> Result<RegKey, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (prefs_key, _) = hkcu
        .create_subkey(APP_PREFS_REG_PATH)
        .map_err(|e| format!("Failed to open app prefs registry key: {e}"))?;
    Ok(prefs_key)
}

#[cfg(target_os = "windows")]
fn set_windows_autostart(enabled: bool) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Ok(());
    }

    let run_key = windows_run_key()?;
    if enabled {
        let exe_path_quoted = startup_exe_command()?;
        run_key
            .set_value(STARTUP_REG_VALUE_NAME, &exe_path_quoted)
            .map_err(|e| format!("Failed to set startup registry value: {e}"))?;
    } else {
        let _ = run_key.delete_value(STARTUP_REG_VALUE_NAME);
    }

    let pref_value: u32 = if enabled { 1 } else { 0 };
    windows_prefs_key()?
        .set_value(APP_PREF_STARTUP_VALUE_NAME, &pref_value)
        .map_err(|e| format!("Failed to persist startup preference: {e}"))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn set_windows_autostart(_enabled: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn launch_at_startup_enabled() -> Result<bool, String> {
    if cfg!(debug_assertions) {
        return Ok(false);
    }

    let prefs_key = windows_prefs_key()?;
    let pref: Result<u32, _> = prefs_key.get_value(APP_PREF_STARTUP_VALUE_NAME);

    let enabled = match pref {
        Ok(value) => value != 0,
        Err(_) => true,
    };

    set_windows_autostart(enabled)?;
    Ok(enabled)
}

#[cfg(not(target_os = "windows"))]
fn launch_at_startup_enabled() -> Result<bool, String> {
    Ok(false)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = place_window_bottom_right(app, &window);
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let launch_at_startup = match launch_at_startup_enabled() {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("[TickNest] Startup setup failed: {err}");
                    false
                }
            };

            let main_window = app
                .get_webview_window("main")
                .expect("main window not found");
            let app_handle = app.handle().clone();

            // Initial positioning
            place_window_bottom_right(&app_handle, &main_window)?;

            // Tray icon setup (use explicit asset for reliable updates)
            let icon = tauri::include_image!("icons/32x32.png");
            let startup_item = CheckMenuItem::with_id(
                app,
                STARTUP_MENU_ID,
                "Launch at startup",
                true,
                launch_at_startup,
                None::<&str>,
            )?;
            let startup_item_for_events = startup_item.clone();
            let exit_item = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&startup_item, &exit_item])?;
            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&tray_menu)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = place_window_bottom_right(&app, &window);
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .on_menu_event(move |app, event| {
                    let event_id = event.id().as_ref();
                    if event_id == STARTUP_MENU_ID {
                        let currently_enabled = launch_at_startup_enabled().unwrap_or(false);
                        let next_value = !currently_enabled;

                        if let Err(err) = set_windows_autostart(next_value) {
                            eprintln!("[TickNest] Failed to update startup setting: {err}");
                        }

                        let _ = startup_item_for_events.set_checked(next_value);
                    } else if event_id == "exit" {
                        app.exit(0);
                    }
                })
                .build(app)?;

            let app_handle_for_events = app_handle.clone();
            let window_for_events = main_window.clone();
            let auto_hide_enabled = !env_flag_enabled("TICKNEST_DISABLE_AUTO_HIDE");
            let startup_instant = Instant::now();
            main_window.on_window_event(move |event| match event {
                WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Resized(_) => {
                    let _ = place_window_bottom_right(&app_handle_for_events, &window_for_events);
                }
                WindowEvent::Focused(false) if auto_hide_enabled => {
                    if startup_instant.elapsed() < Duration::from_millis(900) {
                        return;
                    }
                    let _ = window_for_events.hide();
                }
                _ => {}
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            hide_window,
            load_tasks,
            save_tasks
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
