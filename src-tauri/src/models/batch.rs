use crate::models::query::{MatchResult, QueryResult, ValidityResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchInput {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct BatchRow {
    pub code: String,
    pub name: String,
    pub validity: String,
    pub cnas: String,
    pub cma_file: String,
    pub cma_api: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgress {
    pub current: usize,
    pub total: usize,
    pub code: String,
    pub percent: f64,
    pub done: bool,
    pub paused: bool,
    pub warning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItemResult {
    pub code: String,
    pub validity: ValidityResult,
    pub cnas: MatchResult,
    pub cma_file: MatchResult,
    pub cma_api: QueryResult,
}

#[derive(Debug, Default)]
pub struct BatchControl {
    pub paused: bool,
}
