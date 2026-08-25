use tauri::State;

use crate::models::{AppConfig, BrowseEntry, FileInfo, MatchResult, QueryResult, ValidityResult};
use crate::services::{cma_api, config_service, samr_status, standard_parser};
use crate::AppState;

#[tauri::command]
pub async fn query_validity(
    std_code: String,
    samr_url: String,
) -> ValidityResult {
    let raw = std_code.trim();

    if !standard_parser::contains_code(raw) {
        return samr_status::query_by_name(raw, &samr_url).await;
    }

    let code = standard_parser::extract_code(&std_code);
    samr_status::query(&code, &samr_url).await
}

#[tauri::command]
pub async fn query_cma_api(std_code: String, base_url: String) -> QueryResult {
    let raw = std_code.trim();
    if !standard_parser::contains_code(raw) {
        return cma_api::query_by_name(raw, &base_url).await;
    }
    let code = standard_parser::extract_code(&std_code);
    cma_api::query(&code, &base_url).await
}

#[tauri::command]
pub fn query_cnas(std_code: String, state: State<'_, AppState>) -> MatchResult {
    let raw = std_code.trim();
    let matcher = state
        .matcher
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !standard_parser::contains_code(raw) {
        return matcher.query_cnas_by_name(raw);
    }
    let code = standard_parser::extract_code(&std_code);
    matcher.query_cnas(&code)
}

#[tauri::command]
pub fn query_cma_file(std_code: String, state: State<'_, AppState>) -> MatchResult {
    let raw = std_code.trim();
    let matcher = state
        .matcher
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !standard_parser::contains_code(raw) {
        return matcher.query_cma_by_name(raw);
    }
    let code = standard_parser::extract_code(&std_code);
    matcher.query_cma(&code)
}

#[tauri::command]
pub fn load_cnas_file(path: String, state: State<'_, AppState>) -> Result<Vec<FileInfo>, String> {
    let mut matcher = state
        .matcher
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    matcher.add_cnas(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_cma_file(path: String, state: State<'_, AppState>) -> Result<Vec<FileInfo>, String> {
    let mut matcher = state
        .matcher
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    matcher.add_cma(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_cnas_file(index: usize, state: State<'_, AppState>) -> Vec<FileInfo> {
    let mut matcher = state
        .matcher
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    matcher.remove_cnas(index)
}

#[tauri::command]
pub fn remove_cma_file(index: usize, state: State<'_, AppState>) -> Vec<FileInfo> {
    let mut matcher = state
        .matcher
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    matcher.remove_cma(index)
}

#[tauri::command]
pub fn restore_state(state: State<'_, AppState>) -> (Vec<FileInfo>, Vec<FileInfo>) {
    let mut matcher = state
        .matcher
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    matcher.restore_state();
    (matcher.cnas_infos(), matcher.cma_infos())
}

#[tauri::command]
pub fn get_config() -> AppConfig {
    config_service::load()
}

#[tauri::command]
pub fn save_config(cma_url: String, samr_url: String) -> Result<(), String> {
    let cfg = AppConfig { cma_url, samr_url };
    config_service::save(&cfg).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_standards(state: State<'_, AppState>) -> Vec<BrowseEntry> {
    let matcher = state
        .matcher
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    matcher.get_all_entries()
}

#[tauri::command]
pub fn open_pdf_at_page(path: String, page: u32) -> Result<(), String> {
    let normalized = path.replace('\\', "/");
    let encoded_path = normalized
        .split('/')
        .map(|segment| {
            // Keep drive letter (e.g. "C:") intact without %3A
            if segment.ends_with(':') && segment.len() == 2 {
                segment.to_string()
            } else {
                urlencoding::encode(segment).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    let url = format!("file:///{}#page={}", encoded_path, page);
    std::process::Command::new("cmd")
        .args(["/c", "start", "", &url])
        .spawn()
        .map_err(|e| format!("打开PDF页面失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    std::process::Command::new("cmd")
        .args(["/c", "start", "", &url])
        .spawn()
        .map_err(|e| format!("打开链接失败: {}", e))?;
    Ok(())
}
