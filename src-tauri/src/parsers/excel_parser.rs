use calamine::{open_workbook_auto, Reader};
use regex::Regex;

use crate::services::local_matcher::StandardEntry;
use crate::services::standard_parser;

pub fn parse(path: &str) -> Result<Vec<StandardEntry>, String> {
    let mut workbook = open_workbook_auto(path).map_err(|e| format!("打开Excel失败：{}", e))?;

    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let std_re = Regex::new(r"([A-Za-z]+[/]?[A-Za-z]*)\s*([0-9]+(?:[.\-][0-9]+)*)\s*[-\u{FF0D}\u{2014}]?\s*([0-9]{4})?").unwrap();

    let sheet_names = workbook.sheet_names().to_vec();
    for name in sheet_names {
        let range = match workbook.worksheet_range(&name) {
            Ok(r) => r,
            Err(_) => continue,
        };

        for row in range.rows() {
            for cell in row {
                let text = cell.to_string();
                if text.is_empty() {
                    continue;
                }
                for cap in std_re.captures_iter(&text) {
                    let prefix = cap[1].replace(' ', "");
                    let number = cap[2].replace(' ', "");
                    let year = cap.get(3).map(|m| m.as_str()).unwrap_or("");

                    if prefix.is_empty() || number.is_empty() {
                        continue;
                    }

                    let code = if year.is_empty() {
                        format!("{} {}", prefix, number)
                    } else {
                        format!("{} {}-{}", prefix, number, year)
                    };

                    let norm = standard_parser::normalize(&code);
                    if !seen.insert(norm) {
                        continue;
                    }

                    let name_part = text.replace(&cap[0], "").trim().to_string();
                    entries.push(StandardEntry { code, name: name_part });
                }
            }
        }
    }

    Ok(entries)
}
