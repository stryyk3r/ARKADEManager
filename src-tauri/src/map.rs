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

/// When a map ID is renamed in settings but the saves folder stays the same,
/// return old_id -> new_id pairs so jobs can be migrated.
pub fn find_map_id_renames(
    old_maps: &[MapDefinition],
    new_maps: &[MapDefinition],
) -> std::collections::HashMap<String, String> {
    use std::collections::{HashMap, HashSet};

    let new_ids: HashSet<&str> = new_maps.iter().map(|m| m.id.as_str()).collect();
    let mut renames = HashMap::new();

    for old in old_maps {
        if new_ids.contains(old.id.as_str()) {
            continue;
        }
        if let Some(new_map) = new_maps.iter().find(|m| {
            m.id != old.id && m.folder_name.trim() == old.folder_name.trim()
        }) {
            renames.insert(old.id.clone(), new_map.id.clone());
        }
    }

    renames
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_map_id_renames_when_id_changes_but_folder_stays_same() {
        let old_maps = vec![map(
            "LostColony",
            "Lost Colony",
            "LostColony_WP",
        )];
        let new_maps = vec![map("LC", "Lost Colony", "LostColony_WP")];

        let renames = find_map_id_renames(&old_maps, &new_maps);
        assert_eq!(renames.get("LostColony"), Some(&"LC".to_string()));
    }

    #[test]
    fn find_map_id_renames_is_empty_when_ids_unchanged() {
        let maps = default_ark_maps();
        let renames = find_map_id_renames(&maps, &maps);
        assert!(renames.is_empty());
    }
}
