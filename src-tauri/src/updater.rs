use std::cmp::Ordering;
use std::path::PathBuf;

use tauri::{AppHandle, Emitter};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const VERSION_URL: &str = "https://update.2005666.xyz/searchapp";
const USER_AGENT: &str = "Searchapp";

#[derive(serde::Serialize, Clone)]
pub struct UpdateInfo {
    pub has_update: bool,
    pub version: String,
    pub notes: String,
    pub url: String,
    pub message: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ProgressPayload {
    pub percent: f64,
    pub downloaded: u64,
    pub total: u64,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .unwrap_or_default()
}

fn compare_versions(a: &str, b: &str) -> Ordering {
    let parse = |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse().ok()).collect() };
    let (pa, pb) = (parse(a), parse(b));
    let len = pa.len().max(pb.len());
    for i in 0..len {
        let (va, vb) = (pa.get(i).copied().unwrap_or(0), pb.get(i).copied().unwrap_or(0));
        match va.cmp(&vb) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

pub async fn check() -> UpdateInfo {
    let http = client();

    let resp = match http.get(VERSION_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            return UpdateInfo {
                has_update: false,
                version: String::new(),
                notes: String::new(),
                url: String::new(),
                message: format!("检查更新失败：{}", e),
            }
        }
    };

    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return UpdateInfo {
                has_update: false,
                version: String::new(),
                notes: String::new(),
                url: String::new(),
                message: format!("解析版本信息失败：{}", e),
            }
        }
    };

    let remote = json["version"].as_str().unwrap_or("").to_string();
    let download_url = json["url"].as_str().unwrap_or("").to_string();
    let notes = json["notes"].as_str().unwrap_or("").to_string();

    if remote.is_empty() {
        return UpdateInfo {
            has_update: false,
            version: remote,
            notes,
            url: download_url,
            message: "版本信息无效".to_string(),
        };
    }

    if compare_versions(&remote, CURRENT_VERSION) != Ordering::Greater {
        return UpdateInfo {
            has_update: false,
            version: remote,
            notes,
            url: download_url,
            message: format!("当前已是最新版本（v{}）", CURRENT_VERSION),
        };
    }

    if download_url.is_empty() {
        return UpdateInfo {
            has_update: false,
            version: remote.clone(),
            notes,
            url: download_url,
            message: format!("发现新版本 v{}，但下载地址无效", remote),
        };
    }

    UpdateInfo {
        has_update: true,
        version: remote.clone(),
        notes: notes.clone(),
        url: download_url,
        message: format!("发现新版本 v{}（{}）", remote, notes),
    }
}

pub async fn download(app: &AppHandle, url: &str) -> Result<(), String> {
    let http = client();

    let resp = http.get(url).send().await.map_err(|e| e.to_string())?;
    let total = resp.content_length().unwrap_or(0);

    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe_path.parent().ok_or_else(|| "无法确定程序目录".to_string())?;
    let temp_path: PathBuf = dir.join("标准综合查询器_new.exe");

    let mut downloaded: u64 = 0;
    let mut file_bytes: Vec<u8> = Vec::new();

    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file_bytes.extend_from_slice(&chunk);
        downloaded += chunk.len() as u64;

        let percent = if total > 0 {
            (downloaded as f64 / total as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        let _ = app.emit("update-progress", ProgressPayload { percent, downloaded, total });
    }

    std::fs::write(&temp_path, &file_bytes).map_err(|e| e.to_string())?;

    let bat_path: PathBuf = dir.join("update.bat");
    let bat = format!(
        "@echo off\r\ntimeout /t 2 /nobreak >nul\r\nmove /y \"{}\" \"{}\" >nul\r\nstart \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
        temp_path.display(),
        exe_path.display(),
        exe_path.display()
    );
    std::fs::write(&bat_path, bat).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn apply() {
    let exe_path = std::env::current_exe().unwrap_or_default();
    let dir = exe_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let bat_path = dir.join("update.bat");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = std::process::Command::new("cmd")
            .args(["/C", bat_path.to_str().unwrap_or("")])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }

    std::process::exit(0);
}
