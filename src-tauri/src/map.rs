use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MapDefinition {
    /// Stored on jobs (e.g. TheIsland)
    pub id: String,
    /// Shown in dropdowns (e.g. The Island)
    pub display_name: String,
    /// Saves folder under SavedArks (e.g. TheIsland_WP or Forglar)
    pub folder_name: String,
    /// Map .ark file name when including map save (e.g. TheIsland_WP.ark)
    pub map_file_name: String,
}

pub fn default_ark_maps() -> Vec<MapDefinition> {
    vec![
        map("TheIsland", "The Island", "TheIsland_WP"),
        map("TheCenter", "The Center", "TheCenter_WP"),
        map("ScorchedEarth", "Scorched Earth", "ScorchedEarth_WP"),
        map("Ragnarok", "Ragnarok", "Ragnarok_WP"),
        map("Aberration", "Aberration", "Aberration_WP"),
        map("Extinction", "Extinction", "Extinction_WP"),
        map("Valguero", "Valguero", "Valguero_WP"),
        map("Svartalfheim", "Svartalfheim", "Svartalfheim"),
        map("Astraeos", "Astraeos", "Astraeos_WP"),
        map("Forglar", "Forglar", "Forglar"),
        map("Amissa", "Amissa", "Amissa"),
        map("LostColony", "Lost Colony", "LostColony_WP"),
    ]
}

fn map(id: &str, display_name: &str, folder_name: &str) -> MapDefinition {
    MapDefinition {
        id: id.to_string(),
        display_name: display_name.to_string(),
        folder_name: folder_name.to_string(),
        map_file_name: format!("{}_WP.ark", id),
    }
}

pub fn resolve_map<'a>(maps: &'a [MapDefinition], id: &str) -> Option<&'a MapDefinition> {
    maps.iter().find(|m| m.id == id)
}

pub fn validate_maps(maps: &[MapDefinition]) -> Result<(), String> {
    if maps.is_empty() {
        return Err("At least one map is required".to_string());
    }

    let mut seen = std::collections::HashSet::new();
    for map in maps {
        let id = map.id.trim();
        let display_name = map.display_name.trim();
        let folder_name = map.folder_name.trim();
        let map_file_name = map.map_file_name.trim();

        if id.is_empty() {
            return Err("Each map needs an ID".to_string());
        }
        if display_name.is_empty() {
            return Err(format!("Map \"{}\" needs a display name", id));
        }
        if folder_name.is_empty() {
            return Err(format!("Map \"{}\" needs a saves folder name", id));
        }
        if map_file_name.is_empty() {
            return Err(format!("Map \"{}\" needs a map file name", id));
        }
        if !seen.insert(id.to_string()) {
            return Err(format!("Duplicate map ID: {}", id));
        }
    }

    Ok(())
}
