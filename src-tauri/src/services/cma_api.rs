use serde::Serialize;

use super::standard_parser;

#[derive(Serialize, Clone)]
pub struct QueryResult {
    pub status: String,
    pub message: String,
}

async fn fetch_rows(param: &str, value: &str, base_url: &str) -> Result<(Vec<serde_json::Value>, u64), String> {
    let clean = value.replace(' ', "");
    let encoded = urlencoding::encode(&clean);
    let url = format!(
        "{}/cma-admin/system/standardData/list?pageNum=1&pageSize=20&{}={}",
        base_url.trim_end_matches('/'),
        param,
        encoded
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Referer", "https://cma.caqit.org.cn/")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
        .map_err(|e| format!("请求失败：{}", e))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("解析响应失败：{}", e))?;

    if json["code"].as_i64() != Some(200) {
        return Err("接口返回异常".to_string());
    }

    let rows = json["rows"].as_array().cloned().unwrap_or_default();
    let total = json["total"].as_u64().unwrap_or(rows.len() as u64);
    Ok((rows, total))
}

fn row_lines(rows: &[serde_json::Value]) -> Vec<String> {
    rows.iter()
        .map(|r| {
            format!(
                "{} {}",
                r["standardCode"].as_str().unwrap_or(""),
                r["standardMethod"].as_str().unwrap_or("")
            )
        })
        .collect()
}

fn more_notice(total: u64, shown: usize) -> String {
    if total > shown as u64 {
        format!(
            "\n共 {} 条结果，当前仅显示第 1 页的 {} 条，更多结果请前往 CMA 能力项目库网站查看",
            total, shown
        )
    } else {
        String::new()
    }
}

pub async fn query_by_name(std_name: &str, base_url: &str) -> QueryResult {
    let (rows, total) = match fetch_rows("standardMethod", std_name, base_url).await {
        Ok(r) => r,
        Err(e) => {
            return QueryResult {
                status: "error".into(),
                message: e,
            }
        }
    };

    if rows.is_empty() {
        return QueryResult {
            status: "nomatch".into(),
            message: "无匹配".into(),
        };
    }

    QueryResult {
        status: "partial".into(),
        message: format!(
            "按名称找到 {} 条相关标准。\n库中标准为：{}{}",
            total,
            row_lines(&rows).join("\n"),
            more_notice(total, rows.len())
        ),
    }
}

/// GYT222 -> GY/T222（CMA 接口不做斜杠归一化，需补斜杠变体重试）
fn slash_variant(code: &str) -> Option<String> {
    let chars: Vec<char> = code.chars().collect();
    if chars.len() >= 3
        && chars[0].is_ascii_alphabetic()
        && chars[1].is_ascii_alphabetic()
        && chars[2].is_ascii_alphabetic()
    {
        let mut v: String = chars[..2].iter().collect();
        v.push('/');
        v.extend(&chars[2..]);
        Some(v)
    } else {
        None
    }
}

pub async fn query(std_code: &str, base_url: &str) -> QueryResult {
    let (mut rows, mut total) = match fetch_rows("standardCode", std_code, base_url).await {
        Ok(r) => r,
        Err(e) => {
            return QueryResult {
                status: "error".into(),
                message: e,
            }
        }
    };

    if rows.is_empty() {
        if let Some(variant) = slash_variant(&std_code.replace(' ', "")) {
            if let Ok(r) = fetch_rows("standardCode", &variant, base_url).await {
                rows = r.0;
                total = r.1;
            }
        }
    }

    if rows.is_empty() {
        return QueryResult {
            status: "nomatch".into(),
            message: "无匹配".into(),
        };
    }

    let target_norm = standard_parser::normalize(std_code);

    for row in &rows {
        let r_code = row["standardCode"].as_str().unwrap_or("");
        let r_name = row["standardMethod"].as_str().unwrap_or("");
        let remark = row["remark"].as_str().unwrap_or("");

        if standard_parser::normalize(r_code) == target_norm {
            let mut msg = format!("完全匹配：{} {}", r_code, r_name);
            if !remark.is_empty() {
                msg.push_str(&format!("\n备注：{}", remark));
            }
            return QueryResult {
                status: "exact".into(),
                message: msg,
            };
        }
    }

    if !rows.is_empty() {
        return QueryResult {
            status: "partial".into(),
            message: format!(
                "未完全匹配。\n库中标准为：{}{}",
                row_lines(&rows).join("\n"),
                more_notice(total, rows.len())
            ),
        };
    }

    QueryResult {
        status: "nomatch".into(),
        message: "无匹配".into(),
    }
}
