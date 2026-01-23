use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: Option<String>,
    pub monthly_archive_destination: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Some("light".to_string()),
            monthly_archive_destination: Some(
                r"C:\arkade\Arkade Shared Global\FOTM Backups".to_string(),
            ),
        }
    }
}

