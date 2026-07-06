use crate::app_data::AppData;
use crate::job::Job;
use crate::validation::derive_saves_dir;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct DataLookupMatch {
    pub job_id: String,
    pub job_name: String,
    pub map: String,
    pub root_dir: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: u64,
    pub modified_at: Option<String>,
    pub lookup_type: String,
    pub identifier: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteFileResult {
    pub file_path: String,
    pub success: bool,
    pub error: Option<String>,
}

pub fn normalize_eos_id(identifier: &str) -> Result<String, String> {
    let trimmed = identifier.trim().to_lowercase();
    if trimmed.len() != 32 {
        return Err("EOSID must be exactly 32 characters".to_string());
    }
    if !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("EOSID must contain only hexadecimal characters".to_string());
    }
    Ok(trimmed)
}

pub fn normalize_tribe_id(identifier: &str) -> Result<String, String> {
    let trimmed = identifier.trim();
    if trimmed.is_empty() {
        return Err("TribeID is required".to_string());
    }
    if !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err("TribeID must be a positive integer".to_string());
    }
    let value: u64 = trimmed
        .parse()
        .map_err(|_| "TribeID must be a positive integer".to_string())?;
    if value == 0 {
        return Err("TribeID must be greater than 0".to_string());
    }
    Ok(value.to_string())
}

fn expected_filename(lookup_type: &str, identifier: &str) -> Result<String, String> {
    match lookup_type {
        "profile" => Ok(format!("{}.arkprofile", identifier)),
        "tribe" => Ok(format!("{}.arktribe", identifier)),
        other => Err(format!("Invalid lookup type: {}", other)),
    }
}

fn collect_match(
    job: &Job,
    file_path: &Path,
    lookup_type: &str,
    identifier: &str,
) -> Result<DataLookupMatch, String> {
    let metadata = fs::metadata(file_path).map_err(|e| e.to_string())?;
    let modified_at = metadata.modified().ok().map(|mtime| {
        DateTime::<Utc>::from(mtime).to_rfc3339()
    });

    Ok(DataLookupMatch {
        job_id: job.id.clone(),
        job_name: job.name.clone(),
        map: job.map.clone(),
        root_dir: job.root_dir.clone(),
        file_path: file_path.to_string_lossy().to_string(),
        file_name: file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        file_size: metadata.len(),
        modified_at,
        lookup_type: lookup_type.to_string(),
        identifier: identifier.to_string(),
    })
}

pub fn lookup_data_files(
    app_data: &AppData,
    lookup_type: &str,
    identifier: &str,
) -> Result<Vec<DataLookupMatch>, String> {
    let normalized_id = match lookup_type {
        "profile" => normalize_eos_id(identifier)?,
        "tribe" => normalize_tribe_id(identifier)?,
        other => return Err(format!("Invalid lookup type: {}", other)),
    };

    let filename = expected_filename(lookup_type, &normalized_id)?;
    let jobs = app_data.list_jobs().map_err(|e| e.to_string())?;
    let maps = app_data.get_config().map_err(|e| e.to_string())?.ark_maps();

    let mut matches = Vec::new();

    for job in jobs.iter().filter(|job| job.job_type == "ark") {
        let Some(map) = job.resolve_map(&maps) else {
            continue;
        };

        let saves_dir = derive_saves_dir(&job.root_dir, &map.folder_name);
        let file_path = saves_dir.join(&filename);

        if file_path.is_file() {
            matches.push(collect_match(job, &file_path, lookup_type, &normalized_id)?);
        }
    }

    matches.sort_by(|a, b| {
        a.job_name
            .cmp(&b.job_name)
            .then_with(|| a.file_path.cmp(&b.file_path))
    });

    Ok(matches)
}

fn collect_allowed_saves_dirs(jobs: &[Job], maps: &[crate::map::MapDefinition]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    for job in jobs.iter().filter(|job| job.job_type == "ark") {
        let Some(map) = job.resolve_map(maps) else {
            continue;
        };

        let saves_dir = derive_saves_dir(&job.root_dir, &map.folder_name);
        if let Ok(canonical) = saves_dir.canonicalize() {
            dirs.push(canonical);
        }
    }

    dirs
}

fn is_path_under_allowed_dir(path: &Path, allowed_dirs: &[PathBuf]) -> bool {
    allowed_dirs.iter().any(|allowed| path.starts_with(allowed))
}

fn is_allowed_data_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "arkprofile" | "arktribe"))
        .unwrap_or(false)
}

pub fn delete_data_files(app_data: &AppData, file_paths: &[String]) -> Vec<DeleteFileResult> {
    let jobs = app_data.list_jobs().unwrap_or_default();
    let maps = app_data
        .get_config()
        .map(|c| c.ark_maps())
        .unwrap_or_else(|_| crate::map::default_ark_maps());
    let allowed_dirs = collect_allowed_saves_dirs(&jobs, &maps);

    file_paths
        .iter()
        .map(|file_path| delete_single_file(file_path, &allowed_dirs))
        .collect()
}

fn delete_single_file(file_path: &str, allowed_dirs: &[PathBuf]) -> DeleteFileResult {
    let path = Path::new(file_path);

    let canonical = match path.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            return DeleteFileResult {
                file_path: file_path.to_string(),
                success: false,
                error: Some(format!("File not found or inaccessible: {}", e)),
            };
        }
    };

    if !canonical.is_file() {
        return DeleteFileResult {
            file_path: file_path.to_string(),
            success: false,
            error: Some("Path is not a file".to_string()),
        };
    }

    if !is_allowed_data_file(&canonical) {
        return DeleteFileResult {
            file_path: file_path.to_string(),
            success: false,
            error: Some("Only .arkprofile and .arktribe files can be deleted".to_string()),
        };
    }

    if !is_path_under_allowed_dir(&canonical, allowed_dirs) {
        return DeleteFileResult {
            file_path: file_path.to_string(),
            success: false,
            error: Some("File is not under a configured server saves directory".to_string()),
        };
    }

    match fs::remove_file(&canonical) {
        Ok(()) => DeleteFileResult {
            file_path: file_path.to_string(),
            success: true,
            error: None,
        },
        Err(e) => DeleteFileResult {
            file_path: file_path.to_string(),
            success: false,
            error: Some(format!(
                "Failed to delete file (may be locked by server): {}",
                e
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_eos_id_valid() {
        let id = "ABCDEF0123456789ABCDEF0123456789";
        assert_eq!(
            normalize_eos_id(id).unwrap(),
            "abcdef0123456789abcdef0123456789"
        );
    }

    #[test]
    fn test_normalize_eos_id_rejects_invalid_length() {
        assert!(normalize_eos_id("abc123").is_err());
    }

    #[test]
    fn test_normalize_eos_id_rejects_non_hex() {
        assert!(normalize_eos_id("ghijklmnopqrstuvwxyzabcdefghij").is_err());
    }

    #[test]
    fn test_normalize_tribe_id_valid() {
        assert_eq!(normalize_tribe_id("123456789").unwrap(), "123456789");
    }

    #[test]
    fn test_normalize_tribe_id_rejects_zero() {
        assert!(normalize_tribe_id("0").is_err());
    }

    #[test]
    fn test_normalize_tribe_id_rejects_non_numeric() {
        assert!(normalize_tribe_id("12abc").is_err());
    }

    #[test]
    fn test_is_allowed_data_file() {
        assert!(is_allowed_data_file(Path::new(
            r"C:\test\123.arkprofile"
        )));
        assert!(is_allowed_data_file(Path::new(
            r"C:\test\456.arktribe"
        )));
        assert!(!is_allowed_data_file(Path::new(r"C:\test\map.ark")));
    }
}
