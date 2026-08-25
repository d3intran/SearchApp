use regex::Regex;
use std::sync::LazyLock;

use super::http_client::HTTP_CLIENT;
use super::standard_parser;
use crate::error::{AppError, AppResult};
use crate::models::{ValidityLine, ValidityResult};

static TOTAL_PAGES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"totalPages:(\d+)").unwrap());
static CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<span class="en-code">([\s\S]*?)</span>"#).unwrap());
static NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<span class="en-code">[\s\S]*?</span>([\s\S]*?)</a>"#).unwrap());
static STATUS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<span class="s-status label label-[^"\s]+">([^<]+)</span>"#).unwrap()
});
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

struct ParsedCard {
    code: String,
    name: String,
    status: String,
}

async fn fetch_cards(query: &str, base_url: &str) -> AppResult<(Vec<ParsedCard>, u32)> {
    let clean_query = query.replace(' ', "");
    let encoded = urlencoding::encode(&clean_query);
    let url = format!(
        "{}/search/stdPage?q={}&tid=",
        base_url.trim_end_matches('/'),
        encoded
    );

    let resp = HTTP_CLIENT
        .get(&url)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .send()
        .await
        .map_err(AppError::Network)?;

    let html = resp.text().await.map_err(AppError::Network)?;
    let total_pages = TOTAL_PAGES_RE
        .captures(&html)
        .and_then(|cap| cap[1].parse().ok())
        .unwrap_or(1);
    Ok((parse_cards(&html), total_pages))
}

pub async fn query_by_name(std_name: &str, base_url: &str) -> ValidityResult {
    let (cards, total_pages) = match fetch_cards(std_name, base_url).await {
        Ok(c) => c,
        Err(e) => return error_result(&e.to_string()),
    };

    if cards.is_empty() {
        return ValidityResult {
            found: false,
            lines: vec![ValidityLine {
                text: "无匹配结果".into(),
                color: "red".into(),
            }],
        };
    }

    let mut lines = vec![ValidityLine {
        text: format!("按名称找到 {} 个相关标准：", cards.len()),
        color: "yellow".into(),
    }];
    for c in cards.iter().take(10) {
        let color = if c.status.contains("废止") || c.status.contains("作废") {
            "red"
        } else {
            "green"
        };
        lines.push(ValidityLine {
            text: format!("· {} {}（{}）", c.code, c.name, c.status),
            color: color.into(),
        });
    }
    if total_pages > 1 {
        lines.push(ValidityLine {
            text: format!(
                "共 {} 页结果，当前仅显示第 1 页，更多内容请前往全国标准信息公共服务平台查看",
                total_pages
            ),
            color: "gray".into(),
        });
    }
    ValidityResult { found: true, lines }
}

pub async fn query(std_code: &str, base_url: &str) -> ValidityResult {
    let (cards, total_pages) = match fetch_cards(std_code, base_url).await {
        Ok(c) => c,
        Err(e) => return error_result(&e.to_string()),
    };

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
            let color = if c.status.contains("废止") {
                "red"
            } else {
                "green"
            };
            lines.push(ValidityLine {
                text: format!("· {} {}（{}）", c.code, c.name, c.status),
                color: color.into(),
            });
        }
        if total_pages > 1 {
            lines.push(ValidityLine {
                text: format!(
                    "共 {} 页结果，当前仅显示第 1 页，更多内容请前往全国标准信息公共服务平台查看",
                    total_pages
                ),
                color: "gray".into(),
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
            !std::ptr::eq(*c, matched)
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

    for part in parts.iter().skip(1) {
        let Some(code_cap) = CODE_RE.captures(part) else {
            continue;
        };
        let code = TAG_RE.replace_all(&code_cap[1], "").trim().to_string();

        let name = NAME_RE
            .captures(part)
            .map(|cap| {
                TAG_RE
                    .replace_all(&cap[1], "")
                    .replace("&nbsp;", " ")
                    .trim()
                    .to_string()
            })
            .unwrap_or_default();
        let status = STATUS_RE
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
