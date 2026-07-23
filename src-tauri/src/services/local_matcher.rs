use std::collections::HashMap;

use super::standard_parser;
use crate::parsers;

#[derive(Clone)]
pub struct StandardEntry {
    pub code: String,
    pub name: String,
    pub page: Option<u32>,
}

#[derive(serde::Serialize, Clone)]
pub struct BrowseEntry {
    pub code: String,
    pub name: String,
    pub page: Option<u32>,
    pub source_name: String,
    pub source_path: String,
    pub source_type: String,
}

#[derive(Clone)]
struct LoadedFile {
    path: String,
    name: String,
    entries: Vec<StandardEntry>,
}

#[derive(serde::Serialize, Clone)]
pub struct FileInfo {
    pub name: String,
    pub count: usize,
}

pub struct LocalFileMatcher {
    cnas_files: Vec<LoadedFile>,
    cma_files: Vec<LoadedFile>,
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
            cnas_files: Vec::new(),
            cma_files: Vec::new(),
            cnas_index: HashMap::new(),
            cma_index: HashMap::new(),
        }
    }

    pub fn add_cnas(&mut self, path: &str) -> Result<Vec<FileInfo>, String> {
        parse_and_add(path, &mut self.cnas_files)?;
        rebuild_index(&mut self.cnas_index, &self.cnas_files);
        Ok(self.cnas_infos())
    }

    pub fn add_cma(&mut self, path: &str) -> Result<Vec<FileInfo>, String> {
        parse_and_add(path, &mut self.cma_files)?;
        rebuild_index(&mut self.cma_index, &self.cma_files);
        Ok(self.cma_infos())
    }

    pub fn remove_cnas(&mut self, index: usize) -> Vec<FileInfo> {
        if index < self.cnas_files.len() {
            self.cnas_files.remove(index);
            rebuild_index(&mut self.cnas_index, &self.cnas_files);
        }
        self.cnas_infos()
    }

    pub fn remove_cma(&mut self, index: usize) -> Vec<FileInfo> {
        if index < self.cma_files.len() {
            self.cma_files.remove(index);
            rebuild_index(&mut self.cma_index, &self.cma_files);
        }
        self.cma_infos()
    }

    pub fn cnas_infos(&self) -> Vec<FileInfo> {
        self.cnas_files
            .iter()
            .map(|f| FileInfo { name: f.name.clone(), count: f.entries.len() })
            .collect()
    }

    pub fn cma_infos(&self) -> Vec<FileInfo> {
        self.cma_files
            .iter()
            .map(|f| FileInfo { name: f.name.clone(), count: f.entries.len() })
            .collect()
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

    pub fn get_all_entries(&self) -> Vec<BrowseEntry> {
        let mut result = Vec::new();
        for file in &self.cnas_files {
            for entry in &file.entries {
                result.push(BrowseEntry {
                    code: entry.code.clone(),
                    name: entry.name.clone(),
                    page: entry.page,
                    source_name: file.name.clone(),
                    source_path: file.path.clone(),
                    source_type: "cnas".into(),
                });
            }
        }
        for file in &self.cma_files {
            for entry in &file.entries {
                result.push(BrowseEntry {
                    code: entry.code.clone(),
                    name: entry.name.clone(),
                    page: entry.page,
                    source_name: file.name.clone(),
                    source_path: file.path.clone(),
                    source_type: "cma".into(),
                });
            }
        }
        result
    }
}

fn parse_and_add(path: &str, files: &mut Vec<LoadedFile>) -> Result<(), String> {
    if files.iter().any(|f| f.path == path) {
        return Ok(()); // already loaded, skip
    }
    let entries = parsers::parse_file(path)?;
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string();
    files.push(LoadedFile { path: path.to_string(), name, entries });
    Ok(())
}

fn rebuild_index(index: &mut HashMap<String, Vec<StandardEntry>>, files: &[LoadedFile]) {
    index.clear();
    for file in files {
        for entry in &file.entries {
            let key = standard_parser::normalize(&entry.code);
            if key.is_empty() {
                continue;
            }
            index.entry(key).or_default().push(entry.clone());
        }
    }
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
