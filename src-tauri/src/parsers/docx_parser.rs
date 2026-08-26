use quick_xml::events::Event;
use quick_xml::Reader;
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::LazyLock;
use zip::ZipArchive;

use crate::error::{AppError, AppResult};
use crate::models::StandardEntry;
use crate::services::standard_parser;

static STD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"([A-Za-z]+[/]?[A-Za-z]*)\s*([0-9]+(?:[.\-][0-9]+)*)\s*[-\u{FF0D}\u{2014}]\s*([0-9]{4})",
    )
    .unwrap()
});

static BRACKET_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"《([^》]+)》").unwrap());
static NOISE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"【[^】]*】").unwrap());
static STD_PREFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z]+[/]?[A-Za-z]*\s*[0-9]").unwrap());

pub fn parse(path: &str) -> AppResult<Vec<StandardEntry>> {
    let file = File::open(path).map_err(AppError::Io)?;
    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            // Check if it's a binary .doc format
            return Err(AppError::Word(format!(
                "打开 Word 文档失败（如为老版本二进制 .doc 格式，请在 Word 中另存为 .docx 格式后导入）：{}",
                e
            )));
        }
    };

    let mut all_entries = Vec::new();

    // 1. Parse main document
    if let Ok(mut doc_file) = archive.by_name("word/document.xml") {
        let mut xml = String::new();
        doc_file.read_to_string(&mut xml).map_err(AppError::Io)?;
        let mut entries = parse_document_xml(&xml, "正文");
        all_entries.append(&mut entries);
    } else {
        return Err(AppError::Word(
            "无效的 DOCX 文件：未找到 word/document.xml".to_string(),
        ));
    }

    // 2. Parse headers, footers, footnotes, and endnotes if present
    let extra_names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let file = archive.by_index(i).ok()?;
            let name = file.name();
            if (name.starts_with("word/header")
                || name.starts_with("word/footer")
                || name.starts_with("word/footnotes")
                || name.starts_with("word/endnotes"))
                && name.ends_with(".xml")
            {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();

    for name in extra_names {
        if let Ok(mut extra_file) = archive.by_name(&name) {
            let mut xml = String::new();
            if extra_file.read_to_string(&mut xml).is_ok() {
                let sheet_label = if name.contains("header") {
                    "页眉"
                } else if name.contains("footer") {
                    "页脚"
                } else if name.contains("footnote") {
                    "脚注"
                } else {
                    "尾注"
                };
                let mut entries = parse_document_xml(&xml, sheet_label);
                all_entries.append(&mut entries);
            }
        }
    }

    // 3. Dedup: prefer entries with non-empty name, then smaller row/page number
    let mut best: HashMap<String, StandardEntry> = HashMap::with_capacity(all_entries.len());
    for entry in all_entries {
        let norm = standard_parser::normalize(&entry.code);
        match best.get_mut(&norm) {
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
                    *existing = entry;
                }
            }
        }
    }

    Ok(best.into_values().collect())
}

pub fn parse_document_xml(xml: &str, default_sheet: &str) -> Vec<StandardEntry> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut entries = Vec::new();

    let mut in_table = false;
    let mut in_cell = false;
    let mut in_text = false;

    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();
    let mut current_para = String::new();

    let mut table_idx: usize = 0;
    let mut row_idx: usize = 0;
    let mut para_idx: usize = 0;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.local_name().as_ref() {
                    b"tbl" => {
                        in_table = true;
                        table_idx += 1;
                        row_idx = 0;
                    }
                    b"tr" => {
                        row_idx += 1;
                        current_row.clear();
                    }
                    b"tc" => {
                        in_cell = true;
                        current_cell.clear();
                    }
                    b"p" => {
                        if in_cell && !current_cell.is_empty() && !current_cell.ends_with('\n') {
                            current_cell.push('\n');
                        }
                        if !in_table {
                            current_para.clear();
                        }
                    }
                    b"t" => {
                        in_text = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_text {
                    if let Ok(text) = e.unescape() {
                        if in_cell {
                            current_cell.push_str(&text);
                        } else if !in_table {
                            current_para.push_str(&text);
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                match e.local_name().as_ref() {
                    b"t" => {
                        in_text = false;
                    }
                    b"tc" => {
                        in_cell = false;
                        current_row.push(std::mem::take(&mut current_cell));
                    }
                    b"tr" => {
                        // Process table row (similar to Excel rows)
                        let sheet_name = format!("表格 {}", table_idx);
                        for cell_text in &current_row {
                            if cell_text.is_empty() {
                                continue;
                            }
                            for segment in cell_text.split('\n') {
                                let segment = segment.trim();
                                if segment.is_empty() {
                                    continue;
                                }

                                for cap in STD_RE.captures_iter(segment) {
                                    let prefix = cap[1].replace(' ', "");
                                    let number = &cap[2];
                                    let year = &cap[3];

                                    if prefix.is_empty() || number.is_empty() {
                                        continue;
                                    }

                                    let code = format!("{} {}-{}", prefix, number, year);
                                    let name = extract_name(segment, &cap);

                                    let name = if name.is_empty() {
                                        find_name_in_row(&current_row, cell_text)
                                    } else {
                                        name
                                    };

                                    entries.push(StandardEntry {
                                        code,
                                        name,
                                        page: Some(row_idx as u32),
                                        sheet: sheet_name.clone(),
                                    });
                                }
                            }
                        }
                        current_row.clear();
                    }
                    b"tbl" => {
                        in_table = false;
                    }
                    b"p" if !in_table => {
                        para_idx += 1;
                        let para_text = current_para.trim();
                        if !para_text.is_empty() {
                            for segment in para_text.split('\n') {
                                let segment = segment.trim();
                                if segment.is_empty() {
                                    continue;
                                }
                                for cap in STD_RE.captures_iter(segment) {
                                    let prefix = cap[1].replace(' ', "");
                                    let number = &cap[2];
                                    let year = &cap[3];

                                    if prefix.is_empty() || number.is_empty() {
                                        continue;
                                    }

                                    let code = format!("{} {}-{}", prefix, number, year);
                                    let name = extract_name(segment, &cap);

                                    entries.push(StandardEntry {
                                        code,
                                        name,
                                        page: Some(para_idx as u32),
                                        sheet: default_sheet.to_string(),
                                    });
                                }
                            }
                        }
                        current_para.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("解析 Word XML 警告: {}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    entries
}

fn find_name_in_row(cells: &[String], current_cell: &str) -> String {
    for cell in cells {
        if cell.is_empty() || cell == current_cell {
            continue;
        }
        if STD_RE.is_match(cell) {
            continue;
        }
        let cleaned = NOISE_RE.replace_all(cell, "");
        let cleaned = cleaned.trim();
        if cleaned.chars().count() >= 2
            && cleaned.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
        {
            return cleaned.to_string();
        }
    }
    String::new()
}

fn extract_name(segment: &str, cap: &regex::Captures) -> String {
    let Some(full_match) = cap.get(0) else {
        return String::new();
    };
    let after = &segment[full_match.end()..];
    let before = &segment[..full_match.start()];

    // 《NAME》 after the code
    if let Some(b) = BRACKET_RE.captures(after) {
        let name = b[1].replace('\n', "");
        let name = name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }

    // 《NAME》 before the code
    if let Some(b) = BRACKET_RE.captures_iter(before).last() {
        let name = b[1].replace('\n', "");
        let name = name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }

    // Text after code, strip noise, truncate at next standard code
    let after_clean = NOISE_RE.replace_all(after, "");
    let after_trunc = match STD_PREFIX_RE.find(&after_clean) {
        Some(m) => &after_clean[..m.start()],
        None => &after_clean,
    };
    let after_text = after_trunc
        .trim_matches(|c: char| matches!(c, '、' | '，' | ',' | ' ' | '\u{3000}'))
        .trim();
    if after_text.chars().count() >= 2 {
        return after_text.to_string();
    }

    // Text before code, truncate at previous standard code
    let before_clean = NOISE_RE.replace_all(before, "");
    let before_trunc = match STD_PREFIX_RE.find_iter(&before_clean).last() {
        Some(m) => &before_clean[..m.start()],
        None => &before_clean,
    };
    let before_text = before_trunc
        .trim_matches(|c: char| matches!(c, '、' | '，' | ',' | ' ' | '\u{3000}'))
        .trim();
    if before_text.chars().count() >= 2 {
        return before_text.to_string();
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_docx_xml_table() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body>
                <w:tbl>
                    <w:tr>
                        <w:tc><w:p><w:r><w:t>序号</w:t></w:r></w:p></w:tc>
                        <w:tc><w:p><w:r><w:t>标准编号</w:t></w:r></w:p></w:tc>
                        <w:tc><w:p><w:r><w:t>标准名称</w:t></w:r></w:p></w:tc>
                    </w:tr>
                    <w:tr>
                        <w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc>
                        <w:tc><w:p><w:r><w:t>GB/T 1234-2020</w:t></w:r></w:p></w:tc>
                        <w:tc><w:p><w:r><w:t>广播电视接收设备安全要求</w:t></w:r></w:p></w:tc>
                    </w:tr>
                </w:tbl>
            </w:body>
        </w:document>"#;

        let entries = parse_document_xml(xml, "正文");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, "GB/T 1234-2020");
        assert_eq!(entries[0].name, "广播电视接收设备安全要求");
        assert_eq!(entries[0].sheet, "表格 1");
    }

    #[test]
    fn test_parse_docx_xml_paragraph() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body>
                <w:p>
                    <w:r><w:t>依据标准：GY/T 222-2007 《移动多媒体广播广播信道帧结构》 进行检测。</w:t></w:r>
                </w:p>
            </w:body>
        </w:document>"#;

        let entries = parse_document_xml(xml, "正文");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, "GY/T 222-2007");
        assert_eq!(entries[0].name, "移动多媒体广播广播信道帧结构");
        assert_eq!(entries[0].sheet, "正文");
    }
}
