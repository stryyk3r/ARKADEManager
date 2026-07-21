use crate::config::Config;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub const DEFAULT_ASA_SERVER_ROOT: &str = r"C:\arkservers\asaservers";

pub fn normalize_path_key(path: &str) -> String {
    path.trim().trim_end_matches(['\\', '/']).to_lowercase()
}

pub fn list_child_directories(parent: &str) -> Vec<String> {
    let trimmed = parent.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let path = Path::new(trimmed);
    if !path.is_dir() {
        return Vec::new();
    }

    let mut dirs = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                if let Some(path_str) = entry_path.to_str() {
                    dirs.push(path_str.to_string());
                }
            }
        }
    }

    dirs.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    dirs
}

fn merge_unique_paths(target: &mut Vec<String>, seen: &mut HashSet<String>, paths: impl IntoIterator<Item = String>) {
    for path in paths {
        let key = normalize_path_key(&path);
        if key.is_empty() {
            continue;
        }
        if seen.insert(key) {
            target.push(path.trim_end_matches(['\\', '/']).to_string());
        }
    }
}

pub fn configured_asa_parent(config: &Config) -> String {
    config
        .asa_server_root
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_ASA_SERVER_ROOT)
        .trim_end_matches(['\\', '/'])
        .to_string()
}

pub fn configured_minecraft_parent(config: &Config) -> Option<String> {
    config
        .minecraft_server_root
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches(['\\', '/']).to_string())
}

pub fn configured_palworld_parent(config: &Config) -> Option<String> {
    config
        .palworld_server_root
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches(['\\', '/']).to_string())
}

pub fn collect_asa_server_roots(config: &Config, job_roots: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut roots = Vec::new();

    merge_unique_paths(
        &mut roots,
        &mut seen,
        list_child_directories(&configured_asa_parent(config)),
    );
    merge_unique_paths(&mut roots, &mut seen, job_roots.iter().cloned());

    roots.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    roots
}

pub fn collect_minecraft_server_roots(config: &Config, job_roots: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut roots = Vec::new();

    if let Some(parent) = configured_minecraft_parent(config) {
        merge_unique_paths(&mut roots, &mut seen, list_child_directories(&parent));
    }

    for root in job_roots {
        if root.trim().is_empty() {
            continue;
        }
        merge_unique_paths(&mut roots, &mut seen, std::iter::once(root.clone()));
    }

    roots.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    roots
}

pub fn collect_palworld_server_roots(config: &Config, job_roots: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut roots = Vec::new();

    if let Some(parent) = configured_palworld_parent(config) {
        merge_unique_paths(&mut roots, &mut seen, list_child_directories(&parent));
    }

    for root in job_roots {
        if root.trim().is_empty() {
            continue;
        }
        merge_unique_paths(&mut roots, &mut seen, std::iter::once(root.clone()));
    }

    roots.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    roots
}
