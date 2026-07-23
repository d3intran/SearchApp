use crate::services::{cma_api, local_matcher::{BrowseEntry, FileInfo, MatchResult}, samr_status, standard_parser};
use crate::{config, updater, AppState};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn query_validity(
    app: AppHandle,
    std_code: String,
    samr_url: String,
) -> samr_status::ValidityResult {
    let code = standard_parser::extract_code(&std_code);
    let mut result = samr_status::query(&code, &samr_url).await;

    if !result.found {
        let state = app.state::<AppState>();
        let matcher = state.matcher.lock().unwrap();
        if matcher.is_in_local_files(&code) {
            result.found = true;
            result.lines = vec![
                samr_status::ValidityLine {
                    text: format!("完全匹配，标准为：{}", code),
                    color: "green".into(),
                },
                samr_status::ValidityLine {
                    text: "状态：现行（依据本地附表判定）".into(),
                    color: "green".into(),
                },
            ];
        }
    }

    result
}

#[tauri::command]
pub async fn query_cma_api(std_code: String, base_url: String) -> cma_api::QueryResult {
    let code = standard_parser::extract_code(&std_code);
    cma_api::query(&code, &base_url).await
}

#[tauri::command]
pub fn query_cnas(std_code: String, state: State<'_, AppState>) -> MatchResult {
    let code = standard_parser::extract_code(&std_code);
    let matcher = state.matcher.lock().unwrap();
    matcher.query_cnas(&code)
}

#[tauri::command]
pub fn query_cma_file(std_code: String, state: State<'_, AppState>) -> MatchResult {
    let code = standard_parser::extract_code(&std_code);
    let matcher = state.matcher.lock().unwrap();
    matcher.query_cma(&code)
}

#[tauri::command]
pub fn load_cnas_file(path: String, state: State<'_, AppState>) -> Result<Vec<FileInfo>, String> {
    let mut matcher = state.matcher.lock().unwrap();
    matcher.add_cnas(&path)
}

#[tauri::command]
pub fn load_cma_file(path: String, state: State<'_, AppState>) -> Result<Vec<FileInfo>, String> {
    let mut matcher = state.matcher.lock().unwrap();
    matcher.add_cma(&path)
}

#[tauri::command]
pub fn remove_cnas_file(index: usize, state: State<'_, AppState>) -> Vec<FileInfo> {
    let mut matcher = state.matcher.lock().unwrap();
    matcher.remove_cnas(index)
}

#[tauri::command]
pub fn remove_cma_file(index: usize, state: State<'_, AppState>) -> Vec<FileInfo> {
    let mut matcher = state.matcher.lock().unwrap();
    matcher.remove_cma(index)
}

#[tauri::command]
pub fn get_config() -> config::AppConfig {
    config::load()
}

#[tauri::command]
pub fn save_config(cma_url: String, samr_url: String) -> Result<(), String> {
    let cfg = config::AppConfig { cma_url, samr_url };
    config::save(&cfg)
}

#[tauri::command]
pub async fn check_update() -> updater::UpdateInfo {
    updater::check().await
}

#[tauri::command]
pub async fn download_update(app: AppHandle, url: String) -> Result<(), String> {
    updater::download(&app, &url).await
}

#[tauri::command]
pub fn apply_update() {
    updater::apply();
}

#[tauri::command]
pub fn get_all_standards(state: State<'_, AppState>) -> Vec<BrowseEntry> {
    let matcher = state.matcher.lock().unwrap();
    matcher.get_all_entries()
}

#[tauri::command]
pub fn open_pdf_at_page(path: String, page: u32) -> Result<(), String> {
    let url = format!("file:///{}#page={}", path.replace('\\', "/"), page);
    std::process::Command::new("cmd")
        .args(["/c", "start", "", &url])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
