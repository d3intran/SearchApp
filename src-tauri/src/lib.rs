mod commands;
mod config;
pub mod parsers;
mod services;
mod updater;

use services::local_matcher::LocalFileMatcher;
use std::sync::Mutex;

pub struct AppState {
    pub matcher: Mutex<LocalFileMatcher>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            matcher: Mutex::new(LocalFileMatcher::new()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::query_validity,
            commands::query_cma_api,
            commands::query_cnas,
            commands::query_cma_file,
            commands::load_cnas_file,
            commands::load_cma_file,
            commands::remove_cnas_file,
            commands::remove_cma_file,
            commands::get_config,
            commands::save_config,
            commands::check_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
