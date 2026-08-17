use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalworldRestSettings {
    pub rest_api_enabled: bool,
    pub rest_api_port: u16,
    pub admin_password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArkRconSettings {
    pub rcon_enabled: bool,
    pub rcon_port: Option<u16>,
    pub admin_password: Option<String>,
}

fn strip_quotes(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().trim_matches('"').to_ascii_lowercase().as_str(),
        "true" | "1" | "yes"
    )
}

/// Find the matching close paren for `OptionSettings=(...)`, ignoring parens inside quotes.
fn option_settings_inner(text: &str) -> Option<&str> {
    let marker = "OptionSettings=(";
    let start = text.find(marker)? + marker.len();
    let bytes = text.as_bytes();
    let mut in_quotes = false;
    let mut depth = 0i32;

    for (i, &b) in bytes[start..].iter().enumerate() {
        match b {
            b'"' => in_quotes = !in_quotes,
            b'(' if !in_quotes => depth += 1,
            b')' if !in_quotes => {
                if depth == 0 {
                    return Some(&text[start..start + i]);
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    None
}

/// Parse Palworld `OptionSettings=(Key=Value,...)` from PalWorldSettings.ini text.
pub fn parse_palworld_option_settings(text: &str) -> HashMap<String, String> {
    let Some(inner) = option_settings_inner(text) else {
        return HashMap::new();
    };

    let mut result = HashMap::new();
    let mut key = String::new();
    let mut val = String::new();
    let mut in_key = true;
    let mut in_quotes = false;
    let mut depth = 0i32;

    for ch in inner.chars() {
        if in_key {
            if ch == '=' {
                in_key = false;
            } else {
                key.push(ch);
            }
        } else if ch == '"' {
            in_quotes = !in_quotes;
            val.push(ch);
        } else if ch == '(' && !in_quotes {
            depth += 1;
            val.push(ch);
        } else if ch == ')' && !in_quotes {
            depth -= 1;
            val.push(ch);
        } else if ch == ',' && !in_quotes && depth == 0 {
            result.insert(key.trim().to_string(), val.clone());
            key.clear();
            val.clear();
            in_key = true;
        } else {
            val.push(ch);
        }
    }

    if !key.trim().is_empty() {
        result.insert(key.trim().to_string(), val);
    }

    result
}

pub fn read_palworld_rest_settings(config_dir: &Path) -> Result<PalworldRestSettings> {
    let ini_path = config_dir.join("PalWorldSettings.ini");
    let text = fs::read_to_string(&ini_path)
        .with_context(|| format!("Failed to read {}", ini_path.display()))?;
    parse_palworld_rest_settings_from_text(&text)
        .with_context(|| format!("Invalid PalWorldSettings.ini at {}", ini_path.display()))
}

fn option_value_after_key(text: &str, key: &str) -> Option<String> {
    let marker = format!("{key}=");
    let start = text.find(&marker)? + marker.len();
    let rest = &text[start..];
    let end = rest
        .find([',', ')', '\r', '\n'])
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

pub fn parse_palworld_rest_settings_from_text(text: &str) -> Result<PalworldRestSettings> {
    let options = parse_palworld_option_settings(text);

    let rest_api_enabled = options
        .get("RESTAPIEnabled")
        .map(|v| parse_bool(v))
        .or_else(|| option_value_after_key(text, "RESTAPIEnabled").map(|v| parse_bool(&v)))
        .unwrap_or(false);

    let rest_api_port = options
        .get("RESTAPIPort")
        .and_then(|v| strip_quotes(v).parse::<u16>().ok())
        .or_else(|| {
            option_value_after_key(text, "RESTAPIPort").and_then(|v| strip_quotes(&v).parse::<u16>().ok())
        })
        .unwrap_or(8212);

    let admin_password = options
        .get("AdminPassword")
        .map(|v| strip_quotes(v))
        .or_else(|| option_value_after_key(text, "AdminPassword").map(|v| strip_quotes(&v)))
        .unwrap_or_default();

    Ok(PalworldRestSettings {
        rest_api_enabled,
        rest_api_port,
        admin_password,
    })
}

/// Parse `[ServerSettings]` key/value pairs from GameUserSettings.ini text.
pub fn parse_ark_server_settings(text: &str) -> HashMap<String, String> {
    let mut in_section = false;
    let mut settings = HashMap::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed.eq_ignore_ascii_case("[ServerSettings]");
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        settings.insert(key.trim().to_string(), strip_quotes(value));
    }

    settings
}

pub fn read_ark_rcon_settings(config_dir: &Path) -> Result<ArkRconSettings> {
    let ini_path = config_dir.join("GameUserSettings.ini");
    let text = fs::read_to_string(&ini_path)
        .with_context(|| format!("Failed to read {}", ini_path.display()))?;
    Ok(parse_ark_rcon_settings_from_text(&text))
}

pub fn parse_ark_rcon_settings_from_text(text: &str) -> ArkRconSettings {
    let settings = parse_ark_server_settings(text);

    let rcon_enabled = settings
        .get("RCONEnabled")
        .map(|v| parse_bool(v))
        .unwrap_or(false);

    let rcon_port = settings
        .get("RCONPort")
        .and_then(|v| v.trim().parse::<u16>().ok());

    let admin_password = settings
        .get("ServerAdminPassword")
        .filter(|v| !v.trim().is_empty())
        .cloned();

    ArkRconSettings {
        rcon_enabled,
        rcon_port,
        admin_password,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_palworld_option_settings() {
        let text = r#"[/Script/Pal.PalGameWorldSettings]
OptionSettings=(Difficulty=None,RESTAPIEnabled=True,RESTAPIPort=8212,AdminPassword="secret",RCONEnabled=True)
"#;
        let settings = parse_palworld_rest_settings_from_text(text).unwrap();
        assert!(settings.rest_api_enabled);
        assert_eq!(settings.rest_api_port, 8212);
        assert_eq!(settings.admin_password, "secret");
    }

    #[test]
    fn parses_rest_settings_when_description_contains_parentheses() {
        let text = r#"[/Script/Pal.PalGameWorldSettings]
OptionSettings=(ServerDescription="Join us (discord)",DeathPenalty=All,RCONEnabled=True,RCONPort=37038,RESTAPIEnabled=True,RESTAPIPort=8212,AdminPassword="secret",bIsUseBackupSaveData=True,Region="")
"#;
        let settings = parse_palworld_rest_settings_from_text(text).unwrap();
        assert!(settings.rest_api_enabled);
        assert_eq!(settings.rest_api_port, 8212);
        assert_eq!(settings.admin_password, "secret");
    }

    #[test]
    fn parses_rest_settings_via_fallback_when_quotes_are_unbalanced() {
        let text = r#"[/Script/Pal.PalGameWorldSettings]
OptionSettings=(ServerDescription="broken (no close quote,RESTAPIEnabled=True,RESTAPIPort=8212,AdminPassword="secret")
"#;
        let settings = parse_palworld_rest_settings_from_text(text).unwrap();
        assert!(settings.rest_api_enabled);
        assert_eq!(settings.rest_api_port, 8212);
        assert_eq!(settings.admin_password, "secret");
    }

    #[test]
    fn parses_ark_server_settings() {
        let text = r#"
[ServerSettings]
ServerAdminPassword=ArkAdmin123
RCONEnabled=True
RCONPort=37015
"#;
        let settings = parse_ark_rcon_settings_from_text(text);
        assert!(settings.rcon_enabled);
        assert_eq!(settings.rcon_port, Some(37015));
        assert_eq!(
            settings.admin_password.as_deref(),
            Some("ArkAdmin123")
        );
    }

    #[test]
    fn ark_rcon_disabled_when_missing() {
        let text = "[ServerSettings]\nRCONEnabled=False\n";
        let settings = parse_ark_rcon_settings_from_text(text);
        assert!(!settings.rcon_enabled);
    }
}
