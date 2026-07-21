use serde::Serialize;

use super::standard_parser;

#[derive(Serialize, Clone)]
pub struct QueryResult {
    pub status: String,
    pub message: String,
}

pub async fn query(std_code: &str, base_url: &str) -> QueryResult {
    let query_code = std_code.replace(' ', "");
    let encoded = urlencoding::encode(&query_code);
    let url = format!(
        "{}/cma-admin/system/standardData/list?pageNum=1&pageSize=20&standardCode={}",
        base_url.trim_end_matches('/'),
        encoded
    );

    let client = reqwest::Client::new();
    let resp = match client
        .get(&url)
        .header("Referer", "https://cma.caqit.org.cn/")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return QueryResult {
                status: "error".into(),
                message: format!("请求失败：{}", e),
            }
        }
    };

    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return QueryResult {
                status: "error".into(),
                message: format!("解析响应失败：{}", e),
            }
        }
    };

    if json["code"].as_i64() != Some(200) {
        return QueryResult {
            status: "error".into(),
            message: "接口返回异常".into(),
        };
    }

    let rows = match json["rows"].as_array() {
        Some(r) => r,
        None => {
            return QueryResult {
                status: "nomatch".into(),
                message: "无匹配".into(),
            }
        }
    };

    let target_norm = standard_parser::normalize(std_code);

    for row in rows {
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
        let lines: Vec<String> = rows
            .iter()
            .take(5)
            .map(|r| {
                format!(
                    "{} {}",
                    r["standardCode"].as_str().unwrap_or(""),
                    r["standardMethod"].as_str().unwrap_or("")
                )
            })
            .collect();
        return QueryResult {
            status: "partial".into(),
            message: format!("未完全匹配。\n库中标准为：{}", lines.join("\n")),
        };
    }

    QueryResult {
        status: "nomatch".into(),
        message: "无匹配".into(),
    }
}
