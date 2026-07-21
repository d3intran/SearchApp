pub mod excel_parser;
pub mod pdf_parser;

use crate::services::local_matcher::StandardEntry;

pub fn parse_file(path: &str) -> Result<Vec<StandardEntry>, String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "xlsx" | "xls" => excel_parser::parse(path),
        "pdf" => pdf_parser::parse(path),
        _ => Err(format!("不支持的文件格式：.{}", ext)),
    }
}
