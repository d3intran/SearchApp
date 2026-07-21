use std::cmp::Ordering;
use std::path::PathBuf;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const VERSION_URL: &str = "https://update.2005666.xyz/searchapp";
const USER_AGENT: &str = "Searchapp";

pub struct UpdateOutcome {
    pub message: String,
    pub will_restart: bool,
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

pub async fn check_and_update() -> UpdateOutcome {
    let http = client();

    let resp = match http.get(VERSION_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            return UpdateOutcome {
                message: format!("检查更新失败：{}", e),
                will_restart: false,
            }
        }
    };

    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return UpdateOutcome {
                message: format!("解析版本信息失败：{}", e),
                will_restart: false,
            }
        }
    };

    let remote = json["version"].as_str().unwrap_or("");
    let download_url = json["url"].as_str().unwrap_or("");
    let notes = json["notes"].as_str().unwrap_or("");

    if remote.is_empty() {
        return UpdateOutcome {
            message: "版本信息无效".to_string(),
            will_restart: false,
        };
    }

    if compare_versions(remote, CURRENT_VERSION) != Ordering::Greater {
        return UpdateOutcome {
            message: format!("当前已是最新版本（v{}）", CURRENT_VERSION),
            will_restart: false,
        };
    }

    if download_url.is_empty() {
        return UpdateOutcome {
            message: format!("发现新版本 v{}，但下载地址无效", remote),
            will_restart: false,
        };
    }

    match download_and_replace(&http, download_url).await {
        Ok(_) => UpdateOutcome {
            message: format!("已下载 v{}（{}），即将重启完成更新", remote, notes),
            will_restart: true,
        },
        Err(e) => UpdateOutcome {
            message: format!("发现新版本 v{}，但下载失败：{}", remote, e),
            will_restart: false,
        },
    }
}

async fn download_and_replace(http: &reqwest::Client, url: &str) -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe_path.parent().ok_or_else(|| "无法确定程序目录".to_string())?;
    let temp_path: PathBuf = dir.join("StandardQuery_new.exe");
    let bat_path: PathBuf = dir.join("update.bat");

    let bytes = http
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    std::fs::write(&temp_path, &bytes).map_err(|e| e.to_string())?;

    let bat = format!(
        "@echo off\r\ntimeout /t 2 /nobreak >nul\r\nmove /y \"{}\" \"{}\" >nul\r\nstart \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
        temp_path.display(),
        exe_path.display(),
        exe_path.display()
    );
    std::fs::write(&bat_path, bat).map_err(|e| e.to_string())?;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("cmd")
            .args(["/C", bat_path.to_str().unwrap_or("")])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
