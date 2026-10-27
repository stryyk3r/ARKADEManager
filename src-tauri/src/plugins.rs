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
    /// "Version" from the plugin's PluginInfo.json, when present.
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstalledPlugin {
    /// Folder name without the `_OFF` suffix.
    pub name: String,
    pub version: Option<String>,
    pub disabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DestinationServer {
    pub name: String,
    pub path: String,
    pub plugin_path: String,
    #[serde(default)]
    pub plugins_dir_exists: bool,
    #[serde(default)]
    pub installed: Vec<InstalledPlugin>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstallResult {
    pub files_copied: usize,
    pub files_overwritten: usize,
    /// Existing config.json files left in place because the caller asked to keep them.
    #[serde(default)]
    pub files_skipped: usize,
    pub errors: Vec<String>,
}

/// Each plugin's own settings file, which installs can leave untouched.
const PLUGIN_CONFIG_FILE: &str = "config.json";

/// Read "Version" from `PluginInfo.json` in a plugin folder. ArkApi writes it as a number
/// (e.g. `1.8`) or a string; anything unreadable is treated as unknown.
pub fn read_plugin_version(plugin_dir: &Path) -> Option<String> {
    let content = fs::read_to_string(plugin_dir.join("PluginInfo.json")).ok()?;
    let value: serde_json::Value =
        serde_json::from_str(content.trim_start_matches('\u{feff}')).ok()?;
    let version = value.get("Version").or_else(|| value.get("version"))?;
    let text = match version {
        serde_json::Value::String(s) => s.trim().to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

/// Plugin folders in a server's plugin directory, with versions.
fn list_installed_plugins(plugins_dir: &Path) -> Vec<InstalledPlugin> {
    let mut installed: Vec<InstalledPlugin> = fs::read_dir(plugins_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| {
                    let folder = e.file_name().to_str()?.to_string();
                    let disabled = folder.ends_with("_OFF");
                    let name = folder.strip_suffix("_OFF").unwrap_or(&folder).to_string();
                    Some(InstalledPlugin {
                        name,
                        version: read_plugin_version(&e.path()),
                        disabled,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    installed.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    installed
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
                            
                            let version = read_plugin_version(&entry_path);
                            plugins.push(SourcePlugin { name, path, version });
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

/// True if `name` is a single, ordinary folder name: no separators, drive letters or `.`/`..`.
fn is_plain_folder_name(name: &str) -> bool {
    !name.trim().is_empty()
        && !name.contains(['/', '\\', ':'])
        && matches!(
            Path::new(name).components().collect::<Vec<_>>().as_slice(),
            [std::path::Component::Normal(_)]
        )
}

/// True if `path` contains no `.`/`..` components, so comparing it as text can't be fooled.
fn has_no_relative_components(path: &Path) -> bool {
    !path.components().any(|c| {
        matches!(c, std::path::Component::ParentDir | std::path::Component::CurDir)
    })
}

/// Keys of the plugin directories of all known ASA servers, for checking paths sent by the UI.
pub fn allowed_plugin_dir_keys(roots: &[String]) -> HashSet<String> {
    roots
        .iter()
        .map(|root| server_roots::normalize_path_key(&derive_plugins_dir(root).to_string_lossy()))
        .collect()
}

/// True if `path` is (lexically) one of the allowed plugin directories.
fn is_allowed_plugin_dir(path: &Path, allowed_dir_keys: &HashSet<String>) -> bool {
    has_no_relative_components(path)
        && allowed_dir_keys.contains(&server_roots::normalize_path_key(&path.to_string_lossy()))
}

/// Toggle a plugin folder between enabled and _OFF disabled state.
/// The folder must sit directly inside one of the known servers' plugin directories.
pub fn toggle_plugin_folder(folder_path: &str, allowed_dir_keys: &HashSet<String>) -> Result<String, String> {
    let path = Path::new(folder_path.trim());

    let folder_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| is_plain_folder_name(n))
        .ok_or_else(|| "Invalid folder name".to_string())?;

    let parent = path
        .parent()
        .ok_or_else(|| "Invalid folder path".to_string())?;

    if !is_allowed_plugin_dir(parent, allowed_dir_keys) {
        return Err(format!(
            "Not a plugin folder of a known ARK server: {}",
            folder_path
        ));
    }

    if !path.is_dir() {
        return Err(format!("Folder does not exist: {}", folder_path));
    }

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
    if !is_plain_folder_name(base_folder_name) {
        return Err(format!("Invalid plugin folder name: {}", base_folder_name));
    }

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
                plugins_dir_exists: plugin_path.is_dir(),
                installed: list_installed_plugins(&plugin_path),
            });
        }
    }

    destinations.sort_by(|a, b| a.name.cmp(&b.name));
    destinations
}

/// Copy `source` over `dest` without ever leaving `dest` missing or half-written: copy into a temp
/// file next to it, then rename over the original. If either step fails (e.g. the DLL is locked by
/// the running server), the existing file is left exactly as it was.
fn replace_file(source: &Path, dest: &Path) -> Result<(), String> {
    let file_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("Invalid destination file: {}", dest.display()))?;
    let temp = dest.with_file_name(format!("{}.arkade-tmp", file_name));

    let result = fs::copy(source, &temp)
        .map_err(|e| format!("Failed to copy file {}: {}", source.display(), e))
        .and_then(|_| {
            fs::rename(&temp, dest).map_err(|e| {
                format!(
                    "Failed to replace {} (it may be in use by the running server; the existing file was left unchanged): {}",
                    dest.display(),
                    e
                )
            })
        });
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Copy a folder recursively, tracking overwrites. With `keep_existing_config`, a `config.json`
/// directly inside `destination` that already exists is left as it is.
/// Returns (files copied, files overwritten, files skipped).
fn copy_folder_recursive(
    source: &Path,
    destination: &Path,
    keep_existing_config: bool,
) -> Result<(usize, usize, usize), String> {
    let mut files_copied = 0;
    let mut files_overwritten = 0;
    let mut files_skipped = 0;
    
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
            // file_type() does not follow links. Symlinks/junctions are skipped: following them could
            // copy files from anywhere on disk or recurse forever if one points back up the tree.
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Failed to read entry type {:?}: {}", entry_path, e))?;
            if file_type.is_symlink() {
                log::warn!("Skipping link in plugin source (not copied): {}", entry_path.display());
                continue;
            }

            if file_type.is_dir() {
                let (copied, overwritten, skipped) =
                    copy_folder_recursive(&entry_path, &dest_path, false)?;
                files_copied += copied;
                files_overwritten += overwritten;
                files_skipped += skipped;
            } else {
                let existed = dest_path.exists();
                let is_config = entry_name
                    .to_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case(PLUGIN_CONFIG_FILE));
                if existed && keep_existing_config && is_config {
                    files_skipped += 1;
                    continue;
                }
                if existed {
                    // On Windows, replacing a read-only file fails. Clear read-only so it can be replaced.
                    if let Ok(meta) = fs::metadata(&dest_path) {
                        let mut p = meta.permissions();
                        p.set_readonly(false);
                        let _ = fs::set_permissions(&dest_path, p);
                    }
                }

                replace_file(&entry_path, &dest_path)?;
                if existed {
                    files_overwritten += 1;
                } else {
                    files_copied += 1;
                }
            }
        }
    } else {
        return Err("Source is not a directory".to_string());
    }
    
    Ok((files_copied, files_overwritten, files_skipped))
}

/// Install selected plugins to selected destinations.
/// Every destination must be the plugin directory of a known ASA server.
pub fn install_plugins(
    source_plugin_paths: Vec<String>,
    destination_plugin_paths: Vec<String>,
    keep_existing_config: bool,
    allowed_dir_keys: &HashSet<String>,
) -> Result<InstallResult, String> {
    for dest_plugin_path in &destination_plugin_paths {
        if !is_allowed_plugin_dir(Path::new(dest_plugin_path.trim()), allowed_dir_keys) {
            return Err(format!(
                "Not a plugin folder of a known ARK server: {}",
                dest_plugin_path
            ));
        }
    }

    let mut total_files_copied = 0;
    let mut total_files_overwritten = 0;
    let mut total_files_skipped = 0;
    let mut errors = Vec::new();

    for source_path in &source_plugin_paths {
        let source = Path::new(source_path);

        if !source.is_dir() {
            errors.push(format!("Source plugin folder not found: {}", source_path));
            continue;
        }
        let source_canonical = match source.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                errors.push(format!("Cannot access source plugin {}: {}", source_path, e));
                continue;
            }
        };

        let plugin_name = match source.file_name().and_then(|n| n.to_str()) {
            Some(name) if is_plain_folder_name(name) => name,
            _ => {
                errors.push(format!("Invalid source plugin folder: {}", source_path));
                continue;
            }
        };

        for dest_plugin_path in &destination_plugin_paths {
            let dest_plugin_dir = Path::new(dest_plugin_path.trim());
            let dest_plugin_folder = dest_plugin_dir.join(plugin_name);

            // Copying a folder into itself would recurse forever.
            let dest_inside_source = fs::create_dir_all(dest_plugin_dir)
                .and_then(|_| dest_plugin_dir.canonicalize())
                .map(|dest| dest.starts_with(&source_canonical));
            match dest_inside_source {
                Ok(false) => {}
                Ok(true) => {
                    errors.push(format!(
                        "Cannot install {} into {}: the destination is inside the source folder",
                        plugin_name, dest_plugin_path
                    ));
                    continue;
                }
                Err(e) => {
                    errors.push(format!("Cannot access destination {}: {}", dest_plugin_path, e));
                    continue;
                }
            }

            match copy_folder_recursive(source, &dest_plugin_folder, keep_existing_config) {
                Ok((copied, overwritten, skipped)) => {
                    total_files_copied += copied;
                    total_files_overwritten += overwritten;
                    total_files_skipped += skipped;
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
        files_skipped: total_files_skipped,
        errors,
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    fn setup_server(root: &Path) -> (String, std::path::PathBuf) {
        let plugins_dir = derive_plugins_dir(&root.to_string_lossy());
        fs::create_dir_all(plugins_dir.join("CoolPlugin")).unwrap();
        (root.to_string_lossy().to_string(), plugins_dir)
    }

    #[test]
    fn plain_folder_names() {
        assert!(is_plain_folder_name("CoolPlugin"));
        assert!(is_plain_folder_name("Cool Plugin_OFF"));
        for bad in ["", " ", ".", "..", r"..\x", "a/b", r"a\b", "C:x"] {
            assert!(!is_plain_folder_name(bad), "{:?}", bad);
        }
    }

    #[test]
    fn toggle_only_inside_known_plugin_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let (root, plugins_dir) = setup_server(&temp.path().join("server1"));
        let allowed = allowed_plugin_dir_keys(&[root]);

        let outside = temp.path().join("Important");
        fs::create_dir_all(&outside).unwrap();
        assert!(toggle_plugin_folder(&outside.to_string_lossy(), &allowed).is_err());
        assert!(outside.exists());

        let sneaky = plugins_dir.join("CoolPlugin").join("..").join("..").join("Plugins").join("CoolPlugin");
        assert!(toggle_plugin_folder(&sneaky.to_string_lossy(), &allowed).is_err());

        let renamed = toggle_plugin_folder(&plugins_dir.join("CoolPlugin").to_string_lossy(), &allowed).unwrap();
        assert!(renamed.ends_with("CoolPlugin_OFF"));
    }

    #[test]
    fn replace_file_overwrites_read_only_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let src_dir = temp.path().join("src");
        let dst_dir = temp.path().join("dst");
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&dst_dir).unwrap();
        fs::write(src_dir.join("plugin.dll"), b"new").unwrap();
        let existing = dst_dir.join("plugin.dll");
        fs::write(&existing, b"old").unwrap();
        let mut perms = fs::metadata(&existing).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&existing, perms).unwrap();

        let (copied, overwritten, skipped) = copy_folder_recursive(&src_dir, &dst_dir, false).unwrap();

        assert_eq!((copied, overwritten, skipped), (0, 1, 0));
        assert_eq!(fs::read(&existing).unwrap(), b"new");
        assert!(!dst_dir.join("plugin.dll.arkade-tmp").exists());
    }

    #[cfg(windows)]
    #[test]
    fn failed_replace_keeps_existing_file() {
        use std::os::windows::fs::OpenOptionsExt;
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("new.dll");
        let dest = temp.path().join("plugin.dll");
        fs::write(&source, b"new").unwrap();
        fs::write(&dest, b"old").unwrap();

        // Source locked exclusively, so the copy fails part-way through the install.
        let lock = fs::OpenOptions::new().read(true).share_mode(0).open(&source).unwrap();
        assert!(replace_file(&source, &dest).is_err());
        drop(lock);

        assert_eq!(fs::read(&dest).unwrap(), b"old");
        assert!(!temp.path().join("plugin.dll.arkade-tmp").exists());
    }

    #[test]
    fn toggle_all_rejects_traversal_names() {
        assert!(toggle_plugin_for_all_servers(r"..\..\Windows", true, &[]).is_err());
    }

    #[test]
    fn install_rejects_unknown_destination_and_copies_to_known() {
        let temp = tempfile::tempdir().unwrap();
        let (root, plugins_dir) = setup_server(&temp.path().join("server1"));
        let allowed = allowed_plugin_dir_keys(&[root]);

        let source = temp.path().join("src").join("NewPlugin");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("config.json"), b"{}").unwrap();
        let sources = vec![source.to_string_lossy().to_string()];

        let elsewhere = temp.path().join("elsewhere").to_string_lossy().to_string();
        assert!(install_plugins(sources.clone(), vec![elsewhere], false, &allowed).is_err());

        let result = install_plugins(sources, vec![plugins_dir.to_string_lossy().to_string()], false, &allowed).unwrap();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(plugins_dir.join("NewPlugin").join("config.json").exists());
    }

    #[test]
    fn install_can_keep_existing_top_level_config() {
        let temp = tempfile::tempdir().unwrap();
        let (root, plugins_dir) = setup_server(&temp.path().join("server1"));
        let allowed = allowed_plugin_dir_keys(&[root]);

        let source = temp.path().join("src").join("CoolPlugin");
        fs::create_dir_all(source.join("sub")).unwrap();
        fs::write(source.join("config.json"), b"new").unwrap();
        fs::write(source.join("CoolPlugin.dll"), b"dll").unwrap();
        fs::write(source.join("sub").join("config.json"), b"nested").unwrap();
        let installed = plugins_dir.join("CoolPlugin");
        fs::write(installed.join("Config.json"), b"mine").unwrap();
        let sources = vec![source.to_string_lossy().to_string()];
        let dest = vec![plugins_dir.to_string_lossy().to_string()];

        let result = install_plugins(sources.clone(), dest.clone(), true, &allowed).unwrap();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.files_skipped, 1);
        assert_eq!(fs::read(installed.join("Config.json")).unwrap(), b"mine");
        assert_eq!(fs::read(installed.join("sub").join("config.json")).unwrap(), b"nested");
        assert!(installed.join("CoolPlugin.dll").exists());

        let result = install_plugins(sources, dest, false, &allowed).unwrap();
        assert_eq!(result.files_skipped, 0);
        assert_eq!(fs::read(installed.join("Config.json")).unwrap(), b"new");
    }

    #[test]
    fn plugin_version_reads_number_or_string() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        assert_eq!(read_plugin_version(dir), None);
        fs::write(dir.join("PluginInfo.json"), "\u{feff}{\"FullName\":\"Shop\",\"Version\":1.8}").unwrap();
        assert_eq!(read_plugin_version(dir).as_deref(), Some("1.8"));
        fs::write(dir.join("PluginInfo.json"), r#"{"Version":"2.1.0"}"#).unwrap();
        assert_eq!(read_plugin_version(dir).as_deref(), Some("2.1.0"));
        fs::write(dir.join("PluginInfo.json"), "not json").unwrap();
        assert_eq!(read_plugin_version(dir), None);
    }

    #[test]
    fn installed_plugins_report_disabled_and_versions() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        fs::create_dir_all(dir.join("Beta_OFF")).unwrap();
        fs::create_dir_all(dir.join("alpha")).unwrap();
        fs::write(dir.join("alpha").join("PluginInfo.json"), r#"{"Version":"1.0"}"#).unwrap();
        let installed = list_installed_plugins(dir);
        assert_eq!(installed.len(), 2);
        assert_eq!((installed[0].name.as_str(), installed[0].disabled), ("alpha", false));
        assert_eq!(installed[0].version.as_deref(), Some("1.0"));
        assert_eq!((installed[1].name.as_str(), installed[1].disabled), ("Beta", true));
    }
}
