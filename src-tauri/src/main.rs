use chrono::Local;
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::Manager;

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
        .add_item(tauri::CustomMenuItem::new("open", "Open"))
        .add_item(tauri::CustomMenuItem::new("quit", "Quit"));
    let system_tray = tauri::SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .system_tray(system_tray)
        .setup(|app| {
            let db = Connection::open("connections.db")?;
            db.execute(
                "CREATE TABLE IF NOT EXISTS connections (
                    date TEXT PRIMARY KEY,
                    earliest TEXT NOT NULL,
                    latest TEXT NOT NULL
                )",
                [],
            )?;

            app.manage(AppState { db: Mutex::new(db) });

            // Start background task
            let app_handle = app.handle();
            std::thread::spawn(move || loop {
                if let Err(e) = check_wifi_connection(app_handle.state()) {
                    eprintln!("Error checking WiFi connection: {}", e);
                }
                thread::sleep(Duration::from_secs(60));
            });

            Ok(())
        })
        .on_system_tray_event(|app, event| match event {
            tauri::SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "open" => {
                    if let Some(window) = app.get_window("main") {
                        window.show().unwrap();
                    }
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            },
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![get_connections])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn get_connections(state: tauri::State<AppState>) -> Result<Vec<ConnectionLog>, String> {
    let db = state.db.lock().unwrap();
    let mut stmt = db
        .prepare("SELECT date, earliest, latest FROM connections ORDER BY date DESC")
        .unwrap();
    let logs = stmt
        .query_map([], |row| {
            Ok(ConnectionLog {
                date: row.get(0)?,
                earliest: row.get(1)?,
                latest: row.get(2)?,
            })
        })
        .unwrap();

    logs.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
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
    let target_ssid = "VM5CAC70";
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
