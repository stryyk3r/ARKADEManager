use crate::map::{self, MapDefinition};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: Option<String>,
    pub monthly_archive_destination: Option<String>,
    #[serde(default)]
    pub ark_maps: Option<Vec<MapDefinition>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Some("light".to_string()),
            monthly_archive_destination: Some(
                r"C:\arkade\Arkade Shared Global\FOTM Backups".to_string(),
            ),
            ark_maps: None,
        }
    }
}

impl Config {
    pub fn ark_maps(&self) -> Vec<MapDefinition> {
        self.ark_maps
            .clone()
            .filter(|maps| !maps.is_empty())
            .unwrap_or_else(map::default_ark_maps)
    }
}

