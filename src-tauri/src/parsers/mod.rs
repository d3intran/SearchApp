pub mod docx_parser;
pub mod excel_parser;
pub mod pdf_parser;

use crate::error::{AppError, AppResult};
use crate::models::StandardEntry;

pub fn parse_file(path: &str) -> AppResult<Vec<StandardEntry>> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "xlsx" | "xls" => excel_parser::parse(path),
        "pdf" => pdf_parser::parse(path),
        "docx" | "doc" => docx_parser::parse(path),
        _ => Err(AppError::UnsupportedFormat(ext)),
    }
}
