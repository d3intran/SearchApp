mod batch;
mod commands;
mod config;
pub mod parsers;
pub mod services;

use services::local_matcher::LocalFileMatcher;
use std::sync::Mutex;

pub struct AppState {
    pub matcher: Mutex<LocalFileMatcher>,
    pub batch_inputs: Mutex<Vec<batch::BatchInput>>,
    pub batch_control: Mutex<batch::BatchControl>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            matcher: Mutex::new(LocalFileMatcher::new()),
            batch_inputs: Mutex::new(Vec::new()),
            batch_control: Mutex::new(batch::BatchControl { paused: false }),
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
            commands::restore_state,
            commands::get_config,
            commands::save_config,
            commands::get_all_standards,
            commands::open_pdf_at_page,
            commands::open_url,
            batch::parse_batch_file,
            batch::get_batch_inputs,
            batch::clear_batch_inputs,
            batch::run_batch_query,
            batch::pause_batch_query,
            batch::resume_batch_query,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
