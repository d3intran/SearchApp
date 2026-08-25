use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryStatus {
    Exact,
    Partial,
    Nomatch,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub status: QueryStatus,
    pub message: String,
}

impl MatchResult {
    pub fn exact(message: impl Into<String>) -> Self {
        Self {
            status: QueryStatus::Exact,
            message: message.into(),
        }
    }

    pub fn partial(message: impl Into<String>) -> Self {
        Self {
            status: QueryStatus::Partial,
            message: message.into(),
        }
    }

    pub fn nomatch(message: impl Into<String>) -> Self {
        Self {
            status: QueryStatus::Nomatch,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            status: QueryStatus::Error,
            message: message.into(),
        }
    }
}

pub type QueryResult = MatchResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidityLine {
    pub text: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidityResult {
    pub found: bool,
    pub lines: Vec<ValidityLine>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_status_serialization() {
        assert_eq!(serde_json::to_string(&QueryStatus::Exact).unwrap(), "\"exact\"");
        assert_eq!(serde_json::to_string(&QueryStatus::Partial).unwrap(), "\"partial\"");
        assert_eq!(serde_json::to_string(&QueryStatus::Nomatch).unwrap(), "\"nomatch\"");
        assert_eq!(serde_json::to_string(&QueryStatus::Error).unwrap(), "\"error\"");
    }

    #[test]
    fn test_match_result_helpers() {
        let exact = MatchResult::exact("完全匹配");
        assert_eq!(exact.status, QueryStatus::Exact);
        assert_eq!(exact.message, "完全匹配");

        let err = MatchResult::error("错误");
        assert_eq!(err.status, QueryStatus::Error);
    }
}
