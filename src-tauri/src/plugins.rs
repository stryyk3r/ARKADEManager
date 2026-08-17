use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::config;
use crate::server_roots;
use crate::validation::derive_plugins_dir;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SourcePlugin {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DestinationServer {
    pub name: String,
    pub path: String,
    pub plugin_path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstallResult {
    pub files_copied: usize,
    pub files_overwritten: usize,
    pub errors: Vec<String>,
}

/// List immediate subdirectories of a source folder as installable plugins
pub fn list_source_plugins(source_path: &str) -> Result<Vec<SourcePlugin>, String> {
    let path = Path::new(source_path);
    
    if !path.exists() {
        return Err(format!("Source path does not exist: {}", source_path));
    }
    
    if !path.is_dir() {
        return Err(format!("Source path is not a directory: {}", source_path));
    }
    
    let mut plugins = Vec::new();
    
    match fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let entry_path = entry.path();
                        if entry_path.is_dir() {
                            let name = entry_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown")
                                .to_string();
                            let path = entry_path
                                .to_str()
                                .ok_or_else(|| "Invalid path encoding".to_string())?
                                .to_string();
                            
                            plugins.push(SourcePlugin { name, path });
                        }
                    }
                    Err(e) => {
                        log::warn!("Error reading directory entry: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            return Err(format!("Failed to read source directory: {}", e));
        }
    }
    
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(plugins)
}

/// List plugin folders under a server root's ShooterGame/Plugins directory.
pub fn list_plugin_folders(server_root: &str) -> Result<Vec<serde_json::Value>, String> {
    let plugins_dir = derive_plugins_dir(server_root);

    if !plugins_dir.exists() {
        return Err(format!(
            "Plugins directory does not exist: {}",
            plugins_dir.display()
        ));
    }

    let mut folders = Vec::new();

    match fs::read_dir(&plugins_dir) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        let folder_name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();

                        let is_disabled = folder_name.ends_with("_OFF");
                        let base_name = if is_disabled {
                            folder_name
                                .strip_suffix("_OFF")
                                .unwrap_or(&folder_name)
                                .to_string()
                        } else {
                            folder_name.clone()
                        };

                        folders.push(serde_json::json!({
                            "name": folder_name,
                            "base_name": base_name,
                            "is_disabled": is_disabled,
                            "full_path": path.to_string_lossy().to_string()
                        }));
                    }
                }
            }
        }
        Err(e) => {
            return Err(format!("Failed to read plugins directory: {}", e));
        }
    }

    folders.sort_by(|a, b| {
        let a_name = a["base_name"].as_str().unwrap_or("");
        let b_name = b["base_name"].as_str().unwrap_or("");
        a_name.cmp(b_name)
    });

    Ok(folders)
}

/// Toggle a plugin folder between enabled and _OFF disabled state.
pub fn toggle_plugin_folder(folder_path: &str) -> Result<String, String> {
    let path = Path::new(folder_path);

    if !path.exists() {
        return Err(format!("Folder does not exist: {}", folder_path));
    }

    let folder_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid folder name".to_string())?;

    let parent = path
        .parent()
        .ok_or_else(|| "Invalid folder path".to_string())?;

    let new_name = if folder_name.ends_with("_OFF") {
        folder_name
            .strip_suffix("_OFF")
            .unwrap_or(folder_name)
            .to_string()
    } else {
        format!("{}_OFF", folder_name)
    };

    let new_path = parent.join(&new_name);

    fs::rename(path, &new_path).map_err(|e| format!("Failed to rename folder: {}", e))?;

    Ok(new_path.to_string_lossy().to_string())
}

/// Toggle a plugin folder across all ASA server roots.
pub fn toggle_plugin_for_all_servers(
    base_folder_name: &str,
    target_state_disabled: bool,
    roots: &[String],
) -> Result<Vec<String>, String> {
    let target_name = if target_state_disabled {
        format!("{}_OFF", base_folder_name)
    } else {
        base_folder_name.to_string()
    };

    let mut toggled_paths = Vec::new();

    for root in roots {
        let plugins_dir = derive_plugins_dir(root);

        if !plugins_dir.exists() {
            continue;
        }

        let folder_without_off = plugins_dir.join(base_folder_name);
        let folder_with_off = plugins_dir.join(format!("{}_OFF", base_folder_name));
        let target_path = plugins_dir.join(&target_name);

        if target_path.exists() {
            continue;
        }

        let path_to_rename = if folder_without_off.exists() {
            Some(folder_without_off)
        } else if folder_with_off.exists() {
            Some(folder_with_off)
        } else {
            None
        };

        if let Some(path) = path_to_rename {
            if path != target_path {
                if let Err(e) = fs::rename(&path, &target_path) {
                    log::error!(
                        "Failed to rename folder {} to {}: {}",
                        path.display(),
                        target_path.display(),
                        e
                    );
                    continue;
                }

                toggled_paths.push(target_path.to_string_lossy().to_string());
            }
        }
    }

    Ok(toggled_paths)
}

/// Discover ASA plugin destination servers from config and job roots.
pub fn discover_plugin_destinations(
    config: &config::Config,
    job_roots: &[String],
) -> Vec<DestinationServer> {
    let mut seen = HashSet::new();
    let mut destinations = Vec::new();

    let norm = |p: &str| p.to_lowercase();

    for root in server_roots::collect_asa_server_roots(config, job_roots) {
        let plugin_path = derive_plugins_dir(&root);
        let plugin_path_str = plugin_path.to_string_lossy().to_string();
        if seen.insert(norm(&plugin_path_str)) {
            let name = Path::new(&root)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();
            destinations.push(DestinationServer {
                name,
                path: root,
                plugin_path: plugin_path_str,
            });
        }
    }

    destinations.sort_by(|a, b| a.name.cmp(&b.name));
    destinations
}

/// Copy a folder recursively, tracking overwrites
fn copy_folder_recursive(
    source: &Path,
    destination: &Path,
) -> Result<(usize, usize), String> {
    let mut files_copied = 0;
    let mut files_overwritten = 0;
    
    // Create destination directory if it doesn't exist
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create destination directory: {}", e))?;
    }
    fs::create_dir_all(destination)
        .map_err(|e| format!("Failed to create destination directory: {}", e))?;
    
    // Copy all files and subdirectories
    if source.is_dir() {
        for entry in fs::read_dir(source)
            .map_err(|e| format!("Failed to read source directory: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let entry_path = entry.path();
            let entry_name = entry_path
                .file_name()
                .ok_or_else(|| "Invalid entry name".to_string())?;
            let dest_path = destination.join(entry_name);
            
            if entry_path.is_dir() {
                let (copied, overwritten) = copy_folder_recursive(&entry_path, &dest_path)?;
                files_copied += copied;
                files_overwritten += overwritten;
            } else {
                // Check if file already exists
                if dest_path.exists() {
                    files_overwritten += 1;
                    // On Windows, overwriting a read-only file can fail. Clear read-only so the copy can replace it.
                    if let Ok(meta) = fs::metadata(&dest_path) {
                        let mut p = meta.permissions();
                        p.set_readonly(false);
                        let _ = fs::set_permissions(&dest_path, p);
                    }
                } else {
                    files_copied += 1;
                }
                
                if fs::copy(&entry_path, &dest_path).is_err() {
                    // If overwrite failed (e.g. read-only or lock), try remove-then-copy
                    let _ = fs::remove_file(&dest_path);
                    fs::copy(&entry_path, &dest_path)
                        .map_err(|e| format!("Failed to copy file {:?}: {}", entry_path, e))?;
                }
            }
        }
    } else {
        return Err("Source is not a directory".to_string());
    }
    
    Ok((files_copied, files_overwritten))
}

/// Install selected plugins to selected destinations
pub fn install_plugins(
    source_plugin_paths: Vec<String>,
    destination_plugin_paths: Vec<String>,
) -> Result<InstallResult, String> {
    let mut total_files_copied = 0;
    let mut total_files_overwritten = 0;
    let mut errors = Vec::new();
    
    for source_path in &source_plugin_paths {
        let source = Path::new(source_path);
        
        if !source.exists() {
            errors.push(format!("Source plugin not found: {}", source_path));
            continue;
        }
        
        let plugin_name = source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");
        
        for dest_plugin_path in &destination_plugin_paths {
            let dest_plugin_dir = Path::new(dest_plugin_path);
            let dest_plugin_folder = dest_plugin_dir.join(plugin_name);
            
            match copy_folder_recursive(source, &dest_plugin_folder) {
                Ok((copied, overwritten)) => {
                    total_files_copied += copied;
                    total_files_overwritten += overwritten;
                }
                Err(e) => {
                    let error_msg = format!(
                        "Failed to copy {} to {}: {}",
                        plugin_name, dest_plugin_path, e
                    );
                    errors.push(error_msg.clone());
                    log::error!("{}", error_msg);
                }
            }
        }
    }
    
    Ok(InstallResult {
        files_copied: total_files_copied,
        files_overwritten: total_files_overwritten,
        errors,
    })
}

