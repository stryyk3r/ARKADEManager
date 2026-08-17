use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
/// Minecraft retention: keep all backups from the last 48 hours, plus the first
/// backup of each calendar day for the last 30 days. Considers both .zip and .7z
/// files whose filename starts with `job_name_` and has a timestamp suffix YYYYMMDD_HHMMSS.
pub(crate) fn cleanup_minecraft_retention(destination_dir: &str, job_name: &str) -> Result<()> {
    let dir = Path::new(destination_dir);
    if !dir.exists() {
        return Ok(());
    }

    let prefix = format!("{}_", job_name);
    let now = Utc::now();
    let cutoff_48h = now - chrono::Duration::hours(48);
    let cutoff_30d = now - chrono::Duration::days(30);

    #[derive(Clone)]
    struct BackupEntry {
        path: PathBuf,
        dt: DateTime<Utc>,
    }

    let mut entries: Vec<BackupEntry> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(s) => s,
            None => continue,
        };
        if !filename.starts_with(&prefix) {
            continue;
        }
        let rest = filename
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(".zip").or_else(|| s.strip_suffix(".7z")));
        let rest = match rest {
            Some(s) => s,
            None => continue,
        };
        // Expect YYYYMMDD_HHMMSS (15 chars)
        if rest.len() != 15 {
            continue;
        }
        let naive = match NaiveDateTime::parse_from_str(rest, "%Y%m%d_%H%M%S") {
            Ok(t) => t,
            Err(_) => continue,
        };
        let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
        entries.push(BackupEntry { path, dt });
    }

    // Keep: (1) all within last 48h, (2) first backup of each day in last 30 days
    let mut to_keep: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for e in &entries {
        if e.dt >= cutoff_48h {
            to_keep.insert(e.path.clone());
        }
    }
    let older_than_48h: Vec<_> = entries.iter().filter(|e| e.dt < cutoff_48h && e.dt >= cutoff_30d).collect();
    use std::collections::HashMap;
    let mut first_of_day_earliest: HashMap<NaiveDate, &BackupEntry> = HashMap::new();
    for e in &older_than_48h {
        let date = e.dt.naive_utc().date();
        first_of_day_earliest
            .entry(date)
            .and_modify(|prev| {
                if e.dt < prev.dt {
                    *prev = e;
                }
            })
            .or_insert(e);
    }
    for e in first_of_day_earliest.values() {
        to_keep.insert((*e).path.clone());
    }

    for e in &entries {
        if !to_keep.contains(&e.path) {
            fs::remove_file(&e.path)
                .with_context(|| format!("Failed to delete old Minecraft backup: {}", e.path.display()))?;
            log::info!("Deleted old Minecraft backup: {}", e.path.display());
        }
    }

    Ok(())
}

pub(crate) fn cleanup_old_backups(destination_dir: &str, retention_days: u32) -> Result<()> {
    let dir = Path::new(destination_dir);
    if !dir.exists() {
        return Ok(());
    }

    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|e| e.to_str()) == Some("zip") {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    let modified_dt: DateTime<Utc> = modified.into();
                    if modified_dt < cutoff {
                        fs::remove_file(&path)
                            .with_context(|| format!("Failed to delete old backup: {}", path.display()))?;
                        log::info!("Deleted old backup: {}", path.display());
                    }
                }
            }
        }
    }

    Ok(())
}
