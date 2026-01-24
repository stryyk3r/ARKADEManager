use std::fs;
use std::path::Path;

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

/// Discover ARK server destinations by scanning C:\arkservers\asaservers
pub fn discover_destinations() -> Result<Vec<DestinationServer>, String> {
    let root_path = Path::new(r"C:\arkservers\asaservers");
    
    if !root_path.exists() {
        return Ok(Vec::new()); // Return empty list if root doesn't exist
    }
    
    let mut destinations = Vec::new();
    
    match fs::read_dir(root_path) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let server_path = entry.path();
                        if server_path.is_dir() {
                            let plugin_path = server_path
                                .join("ShooterGame")
                                .join("Binaries")
                                .join("Win64")
                                .join("ArkApi")
                                .join("Plugins");
                            
                            // Only include if the plugin directory exists or can be created
                            if plugin_path.exists() || plugin_path.parent().is_some() {
                                let name = server_path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("Unknown")
                                    .to_string();
                                let server_path_str = server_path
                                    .to_str()
                                    .ok_or_else(|| "Invalid path encoding".to_string())?
                                    .to_string();
                                let plugin_path_str = plugin_path
                                    .to_str()
                                    .ok_or_else(|| "Invalid path encoding".to_string())?
                                    .to_string();
                                
                                destinations.push(DestinationServer {
                                    name,
                                    path: server_path_str,
                                    plugin_path: plugin_path_str,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Error reading server directory entry: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            return Err(format!("Failed to read servers directory: {}", e));
        }
    }
    
    destinations.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(destinations)
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

