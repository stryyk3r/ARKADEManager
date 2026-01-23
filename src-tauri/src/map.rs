use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Map {
    TheIsland,
    TheCenter,
    ScorchedEarth,
    Ragnarok,
    Aberration,
    Extinction,
    Valguero,
    Svartalfheim,
    Astraeos,
    Forglar,
    Amissa,
    LostColony,
}

impl Map {
    pub fn folder_name(&self) -> &'static str {
        match self {
            Map::TheIsland => "TheIsland_WP",
            Map::TheCenter => "TheCenter_WP",
            Map::ScorchedEarth => "ScorchedEarth_WP",
            Map::Ragnarok => "Ragnarok_WP",
            Map::Aberration => "Aberration_WP",
            Map::Extinction => "Extinction_WP",
            Map::Valguero => "Valguero_WP",
            Map::Svartalfheim => "Svartalfheim_WP",
            Map::Astraeos => "Astraeos_WP",
            Map::Forglar => "Forglar",  // No _WP suffix
            Map::Amissa => "Amissa",    // No _WP suffix
            Map::LostColony => "LostColony_WP",
        }
    }

    #[allow(dead_code)]
    pub fn display_name(&self) -> &'static str {
        match self {
            Map::TheIsland => "The Island",
            Map::TheCenter => "The Center",
            Map::ScorchedEarth => "Scorched Earth",
            Map::Ragnarok => "Ragnarok",
            Map::Aberration => "Aberration",
            Map::Extinction => "Extinction",
            Map::Valguero => "Valguero",
            Map::Svartalfheim => "Svartalfheim",
            Map::Astraeos => "Astraeos",
            Map::Forglar => "Forglar",
            Map::Amissa => "Amissa",
            Map::LostColony => "Lost Colony",
        }
    }

    pub fn base_name(&self) -> &'static str {
        match self {
            Map::TheIsland => "TheIsland",
            Map::TheCenter => "TheCenter",
            Map::ScorchedEarth => "ScorchedEarth",
            Map::Ragnarok => "Ragnarok",
            Map::Aberration => "Aberration",
            Map::Extinction => "Extinction",
            Map::Valguero => "Valguero",
            Map::Svartalfheim => "Svartalfheim",
            Map::Astraeos => "Astraeos",
            Map::Forglar => "Forglar",
            Map::Amissa => "Amissa",
            Map::LostColony => "LostColony",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "TheIsland" => Some(Map::TheIsland),
            "TheCenter" => Some(Map::TheCenter),
            "ScorchedEarth" => Some(Map::ScorchedEarth),
            "Ragnarok" => Some(Map::Ragnarok),
            "Aberration" => Some(Map::Aberration),
            "Extinction" => Some(Map::Extinction),
            "Valguero" => Some(Map::Valguero),
            "Svartalfheim" => Some(Map::Svartalfheim),
            "Astraeos" => Some(Map::Astraeos),
            "Forglar" => Some(Map::Forglar),
            "Amissa" => Some(Map::Amissa),
            "LostColony" => Some(Map::LostColony),
            _ => None,
        }
    }
}

