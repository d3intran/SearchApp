use regex::Regex;

use crate::services::local_matcher::StandardEntry;
use crate::services::standard_parser;

pub fn parse(path: &str) -> Result<Vec<StandardEntry>, String> {
    let text = pdf_extract::extract_text(path).map_err(|e| format!("解析PDF失败：{}", e))?;

    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let std_re = Regex::new(
        r"([A-Za-z]+[/]?[A-Za-z]*)\s+([0-9]+(?:[.\-][0-9]+)*)\s*[-\u{FF0D}\u{2014}]\s*([0-9]{4})"
    ).unwrap();

    for cap in std_re.captures_iter(&text) {
        let prefix = cap[1].replace(' ', "");
        let number = cap[2].to_string();
        let year = cap[3].to_string();

        let code = format!("{} {}-{}", prefix, number, year);
        let norm = standard_parser::normalize(&code);
        if !seen.insert(norm) {
            continue;
        }

        entries.push(StandardEntry {
            code,
            name: String::new(),
        });
    }

    Ok(entries)
}
