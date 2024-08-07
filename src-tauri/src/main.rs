use chrono::Local;
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;
use tauri::Manager;
use tauri::api::path::app_data_dir;
use std::fs;

#[derive(Serialize, Deserialize)]
struct ConnectionLog {
    date: String,
    earliest: String,
    latest: String,
}

struct AppState {
    db: Mutex<Connection>,
}

fn main() {
    // Set up system tray
    let tray_menu = tauri::SystemTrayMenu::new()
        .add_item(tauri::CustomMenuItem::new("toggle", "Show/Hide"))
        .add_item(tauri::CustomMenuItem::new("quit", "Quit"));
    let system_tray = tauri::SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .system_tray(system_tray)
        .setup(|app| {
            let app_handle = app.handle();
            let db = create_db_connection(&app_handle)?;
            app.manage(AppState { db: Mutex::new(db) });

            // Start background task
            std::thread::spawn(move || loop {
                let now = Local::now();
                println!(
                    "Checking WiFi connection at {}",
                    now.format("%Y-%m-%d %H:%M")
                );
                if let Err(e) = check_wifi_connection(app_handle.state()) {
                    eprintln!("Error checking WiFi connection: {}", e);
                }
                thread::sleep(Duration::from_secs(30));
            });

            Ok(())
        })
        .on_system_tray_event(|app, event| match event {
            tauri::SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "toggle" => {
                    let window = app.get_window("main").unwrap();
                    if window.is_visible().unwrap() {
                        window.hide().unwrap();
                    } else {
                        window.show().unwrap();
                        window.set_focus().unwrap();
                        #[cfg(target_os = "macos")]
                        window.set_skip_taskbar(false).unwrap();
                    }
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            },
            _ => {}
        })
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                let window = event.window();
                window.hide().unwrap();
                #[cfg(target_os = "macos")]
                window.set_skip_taskbar(true).unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![get_connections])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| match event {
            _ => {}
        });
}



fn create_db_connection(
    app_handle: &tauri::AppHandle,
) -> Result<Connection, Box<dyn std::error::Error>> {
    let app_data_dir = app_data_dir(&app_handle.config()).expect("Failed to get app data dir");
    fs::create_dir_all(&app_data_dir)?;
    let db_path = app_data_dir.join("connections.db");

    let db = Connection::open(db_path)?;
    db.execute(
        "CREATE TABLE IF NOT EXISTS connections (
            date TEXT PRIMARY KEY,
            earliest TEXT NOT NULL,
            latest TEXT NOT NULL
        )",
        [],
    )?;
    Ok(db)
}

fn get_connection_log(db: MutexGuard<Connection>) -> Result<Vec<ConnectionLog>> {
    let mut stmt =
        db.prepare("SELECT date, earliest, latest FROM connections ORDER BY date DESC")?;
    let logs = stmt.query_map([], |row| {
        Ok(ConnectionLog {
            date: row.get(0)?,
            earliest: row.get(1)?,
            latest: row.get(2)?,
        })
    })?;

    logs.collect()
}

#[tauri::command]
fn get_connections(state: tauri::State<AppState>) -> Result<Vec<ConnectionLog>, String> {
    // return Ok(vec![]);
    let db = state.db.lock().unwrap();
    get_connection_log(db).map_err(|e| e.to_string())
}

fn get_current_wifi_ssid() -> Option<String> {
    // TODO support Windows
    let output = Command::new("networksetup")
        .args(&["-getairportnetwork", "en0"])
        .output()
        .expect("Failed to execute networksetup command");

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // The output format is typically: "Current Wi-Fi Network: SSID_NAME"
        stdout.split(": ").nth(1).map(|s| s.trim().to_string())
    } else {
        println!(
            "Error executing networksetup command: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        None
    }
}

fn insert_connection(state: &tauri::State<AppState>) -> Result<(), String> {
    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();

    let db = state.db.lock().unwrap();
    return db
        .execute(
            "INSERT INTO connections (date, earliest, latest) 
         VALUES (?1, ?2, ?2) 
         ON CONFLICT(date) DO UPDATE SET 
         earliest = MIN(earliest, ?2),
         latest = MAX(latest, ?2)",
            &[&date, &time],
        )
        .map(|_| ())
        .map_err(|e| e.to_string());
}

fn check_wifi_connection(state: tauri::State<AppState>) -> Result<(), String> {
    let target_ssid = "eduroam";
    if let Some(current_ssid) = get_current_wifi_ssid() {
        println!("Current WiFi SSID: {}", current_ssid);
        if current_ssid == target_ssid {
            println!("SSID matched, inserting connection");
            return insert_connection(&state);
        }
    } else {
        println!("No WiFi connection detected");
    }

    Ok(())
}
