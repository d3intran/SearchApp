use std::collections::HashMap;

use super::standard_parser;
use crate::parsers;

#[derive(Clone)]
pub struct StandardEntry {
    pub code: String,
    pub name: String,
}

pub struct LocalFileMatcher {
    cnas_index: HashMap<String, Vec<StandardEntry>>,
    cma_index: HashMap<String, Vec<StandardEntry>>,
}

#[derive(serde::Serialize, Clone)]
pub struct MatchResult {
    pub status: String,
    pub message: String,
}

impl LocalFileMatcher {
    pub fn new() -> Self {
        Self {
            cnas_index: HashMap::new(),
            cma_index: HashMap::new(),
        }
    }

    pub fn load_cnas(&mut self, path: &str) -> Result<usize, String> {
        let entries = parsers::parse_file(path)?;
        let count = entries.len();
        self.cnas_index = build_index(entries);
        Ok(count)
    }

    pub fn load_cma(&mut self, path: &str) -> Result<usize, String> {
        let entries = parsers::parse_file(path)?;
        let count = entries.len();
        self.cma_index = build_index(entries);
        Ok(count)
    }

    pub fn is_in_local_files(&self, std_code: &str) -> bool {
        let norm = standard_parser::normalize(std_code);
        self.cnas_index.contains_key(&norm) || self.cma_index.contains_key(&norm)
    }

    pub fn query_cnas(&self, std_code: &str) -> MatchResult {
        query_index(&self.cnas_index, std_code, "CNAS附表")
    }

    pub fn query_cma(&self, std_code: &str) -> MatchResult {
        query_index(&self.cma_index, std_code, "CMA附表")
    }
}

fn build_index(entries: Vec<StandardEntry>) -> HashMap<String, Vec<StandardEntry>> {
    let mut index: HashMap<String, Vec<StandardEntry>> = HashMap::new();
    for entry in entries {
        let key = standard_parser::normalize(&entry.code);
        if key.is_empty() {
            continue;
        }
        index.entry(key).or_default().push(entry);
    }
    index
}

fn query_index(
    index: &HashMap<String, Vec<StandardEntry>>,
    std_code: &str,
    source_name: &str,
) -> MatchResult {
    if index.is_empty() {
        return MatchResult {
            status: "error".into(),
            message: format!("未加载{}文件，请先选择文件", source_name),
        };
    }

    let target_norm = standard_parser::normalize(std_code);

    if let Some(exact) = index.get(&target_norm) {
        let msg = if exact.len() == 1 {
            format!("匹配唯一结果。\n对应附表标准：{} {}", exact[0].code, exact[0].name)
        } else {
            let lines: Vec<String> = exact.iter().map(|e| format!("{} {}", e.code, e.name)).collect();
            format!("匹配到 {} 条结果。\n{}", exact.len(), lines.join("\n"))
        };
        return MatchResult {
            status: "exact".into(),
            message: msg,
        };
    }

    let partial: Vec<&StandardEntry> = index
        .values()
        .flatten()
        .filter(|e| {
            let norm = standard_parser::normalize(&e.code);
            norm.contains(&target_norm) || target_norm.contains(&norm)
        })
        .take(10)
        .collect();

    if !partial.is_empty() {
        let lines: Vec<String> = partial.iter().map(|e| format!("{} {}", e.code, e.name)).collect();
        return MatchResult {
            status: "partial".into(),
            message: format!("未完全匹配。\n附表中相近标准：\n{}", lines.join("\n")),
        };
    }

    MatchResult {
        status: "nomatch".into(),
        message: "无匹配".into(),
    }
}
