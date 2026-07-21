use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub cma_url: String,
    pub samr_url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cma_url: "https://cma.caqit.org.cn".to_string(),
            samr_url: "https://std.samr.gov.cn".to_string(),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    Some(dir.join("config.json"))
}

pub fn load() -> AppConfig {
    config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(config: &AppConfig) -> Result<(), String> {
    let path = config_path().ok_or_else(|| "无法确定配置文件路径".to_string())?;
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}
