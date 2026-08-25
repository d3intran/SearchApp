use std::path::PathBuf;

use crate::error::{AppError, AppResult};
use crate::models::AppConfig;

fn config_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    Some(dir.join("config.json"))
}

#[must_use]
pub fn load() -> AppConfig {
    config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(config: &AppConfig) -> AppResult<()> {
    let path = config_path().ok_or_else(|| AppError::ConfigPath("无法确定配置文件路径".to_string()))?;
    let json = serde_json::to_string_pretty(config).map_err(AppError::Json)?;
    std::fs::write(path, json).map_err(AppError::Io)
}
