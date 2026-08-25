use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardEntry {
    pub code: String,
    pub name: String,
    pub page: Option<u32>,
    pub sheet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowseEntry {
    pub code: String,
    pub name: String,
    pub page: Option<u32>,
    pub sheet: String,
    pub source_name: String,
    pub source_path: String,
    pub source_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub count: usize,
}
