use calamine::{open_workbook_auto, Reader};
use regex::Regex;
use std::collections::HashMap;

use crate::services::local_matcher::StandardEntry;
use crate::services::standard_parser;

pub fn parse(path: &str) -> Result<Vec<StandardEntry>, String> {
    let mut workbook = open_workbook_auto(path).map_err(|e| format!("打开Excel失败：{}", e))?;

    let std_re = Regex::new(
        r"([A-Za-z]+[/]?[A-Za-z]*)\s*([0-9]+(?:[.\-][0-9]+)*)\s*[-\u{FF0D}\u{2014}]\s*([0-9]{4})",
    )
    .unwrap();
    let bracket_re = Regex::new(r"《([^》]+)》").unwrap();
    let noise_re = Regex::new(r"【[^】]*】").unwrap();
    let std_prefix_re = Regex::new(r"[A-Za-z]+[/]?[A-Za-z]*\s*[0-9]").unwrap();

    // Collect all entries with potential duplicates, then dedup preferring named ones
    let mut all_entries: Vec<StandardEntry> = Vec::new();

    let sheet_names = workbook.sheet_names().to_vec();
    for sheet_name in &sheet_names {
        let range = match workbook.worksheet_range(sheet_name) {
            Ok(r) => r,
            Err(_) => continue,
        };

        for (row_idx, row) in range.rows().enumerate() {
            let row_num = (row_idx + 1) as u32;
            let cells: Vec<String> = row.iter().map(|c| c.to_string()).collect();

            for cell_text in &cells {
                if cell_text.is_empty() {
                    continue;
                }

                for segment in cell_text.split('\n') {
                    let segment = segment.trim();
                    if segment.is_empty() {
                        continue;
                    }

                    for cap in std_re.captures_iter(segment) {
                        let prefix = cap[1].replace(' ', "");
                        let number = &cap[2];
                        let year = &cap[3];

                        if prefix.is_empty() || number.is_empty() {
                            continue;
                        }

                        let code = format!("{} {}-{}", prefix, number, year);
                        let name = extract_name(segment, &cap, &bracket_re, &noise_re, &std_prefix_re);

                        // If no name found in this cell, look at other cells in the same row
                        let name = if name.is_empty() {
                            find_name_in_row(&cells, cell_text, &std_re, &noise_re)
                        } else {
                            name
                        };

                        all_entries.push(StandardEntry {
                            code,
                            name,
                            page: Some(row_num),
                            sheet: sheet_name.clone(),
                        });
                    }
                }
            }
        }
    }

    // Dedup: prefer entries with non-empty name, then smaller row number
    let mut best: HashMap<String, StandardEntry> = HashMap::new();
    for entry in all_entries {
        let norm = standard_parser::normalize(&entry.code);
        match best.get(&norm) {
            None => {
                best.insert(norm, entry);
            }
            Some(existing) => {
                let replace = if existing.name.is_empty() && !entry.name.is_empty() {
                    true
                } else if existing.name.is_empty() == entry.name.is_empty() {
                    entry.page.unwrap_or(u32::MAX) < existing.page.unwrap_or(u32::MAX)
                } else {
                    false
                };
                if replace {
                    best.insert(norm, entry);
                }
            }
        }
    }

    Ok(best.into_values().collect())
}

fn find_name_in_row(
    cells: &[String],
    current_cell: &str,
    std_re: &Regex,
    noise_re: &Regex,
) -> String {
    for cell in cells {
        if cell.is_empty() || cell == current_cell {
            continue;
        }
        // Skip cells that contain standard codes (they are code cells, not name cells)
        if std_re.is_match(cell) {
            continue;
        }
        let cleaned = noise_re.replace_all(cell, "");
        let cleaned = cleaned.trim();
        // A valid name: has Chinese chars, at least 2 chars, no digits-only
        if cleaned.chars().count() >= 2 && cleaned.chars().any(|c| c >= '\u{4e00}' && c <= '\u{9fff}') {
            return cleaned.to_string();
        }
    }
    String::new()
}

fn extract_name(
    segment: &str,
    cap: &regex::Captures,
    bracket_re: &Regex,
    noise_re: &Regex,
    std_prefix_re: &Regex,
) -> String {
    let full_match = cap.get(0).unwrap();
    let after = &segment[full_match.end()..];
    let before = &segment[..full_match.start()];

    // 《NAME》 after the code
    if let Some(b) = bracket_re.captures(after) {
        let name = b[1].replace('\n', "");
        let name = name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }

    // 《NAME》 before the code
    if let Some(b) = bracket_re.captures_iter(before).last() {
        let name = b[1].replace('\n', "");
        let name = name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }

    // Text after code, strip noise, truncate at next standard code
    let after_clean = noise_re.replace_all(after, "");
    let after_trunc = match std_prefix_re.find(&after_clean) {
        Some(m) => &after_clean[..m.start()],
        None => &after_clean,
    };
    let after_text = after_trunc
        .trim_matches(|c: char| c == '、' || c == '，' || c == ',' || c == ' ' || c == '\u{3000}')
        .trim();
    if after_text.chars().count() >= 2 {
        return after_text.to_string();
    }

    // Text before code, truncate at previous standard code
    let before_clean = noise_re.replace_all(before, "");
    let before_trunc = match std_prefix_re.find_iter(&before_clean).last() {
        Some(m) => &before_clean[..m.start()],
        None => &before_clean,
    };
    let before_text = before_trunc
        .trim_matches(|c: char| c == '、' || c == '，' || c == ',' || c == ' ' || c == '\u{3000}')
        .trim();
    if before_text.chars().count() >= 2 {
        return before_text.to_string();
    }

    String::new()
}
