use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};

/// Parse the UTC timestamp from a backup filename of the form `{job_name}_YYYYMMDD_HHMMSS.{ext}`.
/// Returns None for files that don't belong to this job (including jobs whose name merely
/// starts with `job_name`, e.g. `Server_PVE_...` is not a `Server` backup).
fn parse_job_backup_timestamp(job_name: &str, filename: &str, extensions: &[&str]) -> Option<DateTime<Utc>> {
    let rest = filename.strip_prefix(&format!("{}_", job_name))?;
    let rest = extensions
        .iter()
        .find_map(|ext| rest.strip_suffix(&format!(".{}", ext)))?;
    // Expect YYYYMMDD_HHMMSS (15 chars)
    if rest.len() != 15 {
        return None;
    }
    let naive = NaiveDateTime::parse_from_str(rest, "%Y%m%d_%H%M%S").ok()?;
    Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

/// Temp archives older than this can't belong to a backup that is still running.
const STALE_TEMP_HOURS: i64 = 24;

/// Delete this job's leftover `{job_name}_YYYYMMDD_HHMMSS.zip.tmp` / `.7z.tmp` files from runs that
/// were killed before they could clean up (app closed, crash, power loss).
fn cleanup_stale_temp_files(dir: &Path, job_name: &str) -> Result<()> {
    let cutoff = Utc::now() - chrono::Duration::hours(STALE_TEMP_HOURS);
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(s) => s,
            None => continue,
        };
        let dt = match parse_job_backup_timestamp(job_name, filename, &["zip.tmp", "7z.tmp"]) {
            Some(dt) => dt,
            None => continue,
        };
        if dt < cutoff && path.is_file() {
            match fs::remove_file(&path) {
                Ok(()) => log::info!("Deleted stale temp backup file: {}", path.display()),
                Err(e) => log::warn!("Could not delete stale temp backup file {}: {}", path.display(), e),
            }
        }
    }
    Ok(())
}

/// Minecraft retention: keep all backups from the last 48 hours, plus the first
/// backup of each calendar day for the last 30 days. Considers both .zip and .7z
/// files whose filename starts with `job_name_` and has a timestamp suffix YYYYMMDD_HHMMSS.
pub(crate) fn cleanup_minecraft_retention(destination_dir: &str, job_name: &str) -> Result<()> {
    let dir = Path::new(destination_dir);
    if !dir.exists() {
        return Ok(());
    }

    cleanup_stale_temp_files(dir, job_name)?;

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
        let dt = match parse_job_backup_timestamp(job_name, filename, &["zip", "7z"]) {
            Some(dt) => dt,
            None => continue,
        };
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

/// ARK/Palworld retention: delete this job's `{job_name}_YYYYMMDD_HHMMSS.zip` backups older than
/// `retention_days`. Only files matching that exact pattern are touched, so other jobs' backups and
/// unrelated files sharing the destination folder are left alone.
pub(crate) fn cleanup_old_backups(destination_dir: &str, job_name: &str, retention_days: u32) -> Result<()> {
    let dir = Path::new(destination_dir);
    if !dir.exists() {
        return Ok(());
    }

    cleanup_stale_temp_files(dir, job_name)?;

    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(s) => s,
            None => continue,
        };
        let dt = match parse_job_backup_timestamp(job_name, filename, &["zip"]) {
            Some(dt) => dt,
            None => continue,
        };
        if dt < cutoff {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete old backup: {}", path.display()))?;
            log::info!("Deleted old backup: {}", path.display());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, b"x").unwrap();
        p
    }

    fn stamp(days_ago: i64) -> String {
        (Utc::now() - chrono::Duration::days(days_ago))
            .format("%Y%m%d_%H%M%S")
            .to_string()
    }

    #[test]
    fn parse_rejects_other_jobs_with_shared_prefix() {
        assert!(parse_job_backup_timestamp("Server", "Server_20260101_120000.zip", &["zip"]).is_some());
        assert!(parse_job_backup_timestamp("Server", "Server_PVE_20260101_120000.zip", &["zip"]).is_none());
        assert!(parse_job_backup_timestamp("Server", "Server_20260101_120000.zip.tmp", &["zip"]).is_none());
        assert!(parse_job_backup_timestamp("Server", "Server_20260101_120000.7z", &["zip"]).is_none());
    }

    #[test]
    fn cleanup_old_backups_only_deletes_this_jobs_expired_backups() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();

        let old_own = touch(dir, &format!("Island_{}.zip", stamp(10)));
        let new_own = touch(dir, &format!("Island_{}.zip", stamp(1)));
        let old_other_job = touch(dir, &format!("Island_PVP_{}.zip", stamp(10)));
        let old_unrelated = touch(dir, &format!("Ragnarok_{}.zip", stamp(10)));
        let random_zip = touch(dir, "important_stuff.zip");

        cleanup_old_backups(dir.to_str().unwrap(), "Island", 7).unwrap();

        assert!(!old_own.exists());
        assert!(new_own.exists());
        assert!(old_other_job.exists());
        assert!(old_unrelated.exists());
        assert!(random_zip.exists());
    }

    #[test]
    fn cleanup_removes_only_this_jobs_stale_temp_files() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();

        let stale_zip_tmp = touch(dir, &format!("Island_{}.zip.tmp", stamp(2)));
        let stale_7z_tmp = touch(dir, &format!("Island_{}.7z.tmp", stamp(2)));
        let fresh_tmp = touch(dir, &format!("Island_{}.zip.tmp", stamp(0)));
        let other_job_tmp = touch(dir, &format!("Island_PVP_{}.zip.tmp", stamp(2)));

        cleanup_old_backups(dir.to_str().unwrap(), "Island", 30).unwrap();

        assert!(!stale_zip_tmp.exists());
        assert!(!stale_7z_tmp.exists());
        assert!(fresh_tmp.exists());
        assert!(other_job_tmp.exists());
    }
}
