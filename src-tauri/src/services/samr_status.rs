use regex::Regex;
use serde::Serialize;

use super::standard_parser;

#[derive(Serialize, Clone)]
pub struct ValidityLine {
    pub text: String,
    pub color: String,
}

#[derive(Serialize, Clone)]
pub struct ValidityResult {
    pub found: bool,
    pub lines: Vec<ValidityLine>,
}

struct ParsedCard {
    code: String,
    name: String,
    status: String,
}

pub async fn query(std_code: &str, base_url: &str) -> ValidityResult {
    let clean_query = std_code.replace(' ', "");
    let encoded = urlencoding::encode(&clean_query);
    let url = format!(
        "{}/search/stdPage?q={}&tid=",
        base_url.trim_end_matches('/'),
        encoded
    );

    let client = reqwest::Client::new();
    let resp = match client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return error_result(&format!("请求失败：{}", e));
        }
    };

    let html = match resp.text().await {
        Ok(h) => h,
        Err(e) => {
            return error_result(&format!("读取响应失败：{}", e));
        }
    };

    let cards = parse_cards(&html);
    if cards.is_empty() {
        return ValidityResult {
            found: false,
            lines: vec![ValidityLine {
                text: "无匹配结果".into(),
                color: "red".into(),
            }],
        };
    }

    let target_norm = standard_parser::normalize(std_code);

    let exact: Vec<&ParsedCard> = cards
        .iter()
        .filter(|c| standard_parser::normalize(&c.code) == target_norm)
        .collect();

    if let Some(matched) = exact.first() {
        return build_exact_result(matched, &cards);
    }

    let prefix: Vec<&ParsedCard> = cards
        .iter()
        .filter(|c| standard_parser::normalize(&c.code).starts_with(&target_norm))
        .collect();

    if !prefix.is_empty() {
        let mut lines = vec![ValidityLine {
            text: format!("未完全匹配，找到 {} 个相关标准：", prefix.len()),
            color: "yellow".into(),
        }];
        for c in &prefix {
            let color = if c.status.contains("废止") { "red" } else { "green" };
            lines.push(ValidityLine {
                text: format!("· {} {}（{}）", c.code, c.name, c.status),
                color: color.into(),
            });
        }
        return ValidityResult { found: true, lines };
    }

    ValidityResult {
        found: false,
        lines: vec![ValidityLine {
            text: "无匹配结果".into(),
            color: "red".into(),
        }],
    }
}

fn build_exact_result(matched: &ParsedCard, all_cards: &[ParsedCard]) -> ValidityResult {
    let full_name = if matched.name.is_empty() {
        matched.code.clone()
    } else {
        format!("{} {}", matched.code, matched.name)
    };

    let mut lines = vec![
        ValidityLine {
            text: format!("完全匹配，标准为：{}", full_name),
            color: "green".into(),
        },
        ValidityLine {
            text: format!("状态：{}", matched.status),
            color: if matched.status.contains("废止") || matched.status.contains("作废") {
                "red".into()
            } else {
                "green".into()
            },
        },
    ];

    if matched.status.contains("废止") || matched.status.contains("作废") {
        let base = get_base_code(&standard_parser::normalize(&matched.code));
        let replacement = all_cards.iter().find(|c| {
            std::ptr::eq(*c, matched) == false
                && get_base_code(&standard_parser::normalize(&c.code)) == base
                && c.status.contains("现行")
        });

        match replacement {
            Some(r) => {
                let r_name = if r.name.is_empty() {
                    r.code.clone()
                } else {
                    format!("{} {}", r.code, r.name)
                };
                lines.push(ValidityLine {
                    text: format!("被以下现行标准替代：{}", r_name),
                    color: "green".into(),
                });
            }
            None => {
                lines.push(ValidityLine {
                    text: "无替代标准".into(),
                    color: "red".into(),
                });
            }
        }
    }

    ValidityResult { found: true, lines }
}

fn parse_cards(html: &str) -> Vec<ParsedCard> {
    let mut cards = Vec::new();
    let parts: Vec<&str> = html.split("<div class=\"panel panel-default post\">").collect();

    let code_re = Regex::new(r#"<span class="en-code">([\s\S]*?)</span>"#).unwrap();
    let name_re = Regex::new(r#"<span class="en-code">[\s\S]*?</span>(?:&nbsp;)*\s*([^<]+)</a>"#).unwrap();
    let status_re = Regex::new(r#"<span class="s-status label label-[^"\s]+">([^<]+)</span>"#).unwrap();
    let tag_re = Regex::new(r"<[^>]+>").unwrap();

    for part in parts.iter().skip(1) {
        let code = match code_re.captures(part) {
            Some(cap) => tag_re.replace_all(&cap[1], "").trim().to_string(),
            None => continue,
        };
        let name = name_re
            .captures(part)
            .map(|cap| cap[1].trim().to_string())
            .unwrap_or_default();
        let status = status_re
            .captures(part)
            .map(|cap| cap[1].trim().to_string())
            .unwrap_or_else(|| "未知".to_string());

        cards.push(ParsedCard { code, name, status });
    }

    cards
}

fn get_base_code(normalized: &str) -> String {
    if let Some(pos) = normalized.rfind('-') {
        let after = &normalized[pos + 1..];
        if after.len() == 4 && after.chars().all(|c| c.is_ascii_digit()) {
            return normalized[..pos].to_string();
        }
    }
    normalized.to_string()
}

fn error_result(msg: &str) -> ValidityResult {
    ValidityResult {
        found: false,
        lines: vec![ValidityLine {
            text: format!("有效性查询异常：{}", msg),
            color: "red".into(),
        }],
    }
}
