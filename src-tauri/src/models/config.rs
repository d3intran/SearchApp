use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub cma_url: String,
    pub samr_url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cma_url: "https://cma.caqit.org.cn".to_string(),
            samr_url: "https://std.samr.gov.cn".to_string(),
        }
    }
}
