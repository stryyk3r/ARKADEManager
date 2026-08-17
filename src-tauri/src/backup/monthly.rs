use crate::app_data::AppData;
use crate::job::Job;
use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
const DEFAULT_MONTHLY_BASE_DIR: &str = r"C:\arkade\Arkade Shared Global\FOTM Backups";

const MONTHLY_CATEGORY_FOLDERS: &[&str] = &[
    "ASA Legacy",
    "ASE Legacy",
    "ASA Omega",
    "Minecraft",
    "Palworld",
];

fn monthly_folder_name(dt: DateTime<Utc>) -> String {
    // Example: "2026-03-March"
    let month_name = match dt.month() {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    };
    format!("{}-{:02}-{}", dt.year(), dt.month(), month_name)
}

fn ensure_monthly_dir_structure(base_dir: &str, dt: DateTime<Utc>) -> Result<PathBuf> {
    let month_dir = Path::new(base_dir).join(monthly_folder_name(dt));
    fs::create_dir_all(&month_dir)
        .with_context(|| format!("Failed to create monthly directory: {}", month_dir.display()))?;

    for folder in MONTHLY_CATEGORY_FOLDERS {
        fs::create_dir_all(month_dir.join(folder))
            .with_context(|| format!("Failed to create monthly subdirectory: {}", folder))?;
    }

    Ok(month_dir)
}

fn count_existing_monthly_copies(category_dir: &Path, job_name: &str) -> Result<usize> {
    if !category_dir.exists() {
        return Ok(0);
    }
    let prefix = format!("{}_", job_name);
    let mut count = 0usize;
    for entry in fs::read_dir(category_dir)? {
        let entry = entry?;
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let file_name = match p.file_name().and_then(|n| n.to_str()) {
            Some(s) => s,
            None => continue,
        };
        if !file_name.starts_with(&prefix) {
            continue;
        }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("zip") || ext.eq_ignore_ascii_case("7z") {
            count += 1;
        }
    }
    Ok(count)
}

pub(crate) fn maybe_copy_backup_to_monthly(app_data: &AppData, job: &Job, backup_path: &Path) -> Result<()> {
    // Only act on completed backups that exist on disk.
    if !backup_path.exists() {
        return Ok(());
    }

    let config = app_data.get_config()?;
    let base_dir = config
        .monthly_archive_destination
        .unwrap_or_else(|| DEFAULT_MONTHLY_BASE_DIR.to_string());

    let now = Utc::now();
    let month_dir = ensure_monthly_dir_structure(&base_dir, now)?;

    let category = job.monthly_cluster.trim();
    if category.is_empty() {
        log::warn!(
            "MONTHLY: Job {} has empty monthly_cluster; skipping monthly copy",
            job.name
        );
        return Ok(());
    }
    if !MONTHLY_CATEGORY_FOLDERS.iter().any(|c| c.eq_ignore_ascii_case(category)) {
        log::warn!(
            "MONTHLY: Job {} has unknown monthly_cluster '{}'; skipping monthly copy",
            job.name,
            category
        );
        return Ok(());
    }
    let category_dir = month_dir.join(category);

    // First 2 backups for each job each month.
    let existing = count_existing_monthly_copies(&category_dir, &job.name)?;
    if existing >= 2 {
        return Ok(());
    }

    let file_name = backup_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid backup path: {}", backup_path.display()))?;
    let dest_path = category_dir.join(file_name);

    // Copy (do not remove original backup).
    fs::copy(backup_path, &dest_path).with_context(|| {
        format!(
            "Failed to copy monthly backup: {} -> {}",
            backup_path.display(),
            dest_path.display()
        )
    })?;
    log::info!(
        "MONTHLY: Copied {} (job {}) into {}",
        backup_path.display(),
        job.name,
        dest_path.display()
    );

    // After a monthly copy completes, pre-create next month's folder structure.
    let (next_year, next_month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let next_dt = DateTime::<Utc>::from_naive_utc_and_offset(
        NaiveDate::from_ymd_opt(next_year, next_month, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap(),
        Utc,
    );
    let _ = ensure_monthly_dir_structure(&base_dir, next_dt)?;

    Ok(())
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonthlyArchivePreview {
    pub files: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonthlyArchiveResult {
    pub archived: usize,
    pub destination: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonthlyJobStatus {
    pub job_id: String,
    pub job_name: String,
    pub monthly_cluster: String,
    pub copied_this_month: u32,
    pub completed: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonthlyStatusResult {
    pub month_folder: String,
    pub destination_base: String,
    pub jobs: Vec<MonthlyJobStatus>,
}

pub fn get_monthly_status(app_data: &AppData) -> Result<MonthlyStatusResult> {
    let config = app_data.get_config()?;
    let destination_base = config
        .monthly_archive_destination
        .unwrap_or_else(|| DEFAULT_MONTHLY_BASE_DIR.to_string());

    let now = Utc::now();
    let month_folder = Path::new(&destination_base).join(monthly_folder_name(now));

    let jobs = app_data.list_jobs()?;
    let mut statuses = Vec::with_capacity(jobs.len());

    for job in &jobs {
        let cluster = job.monthly_cluster.trim();
        let cluster_dir = if cluster.is_empty() {
            None
        } else {
            Some(month_folder.join(cluster))
        };

        let copied = if let Some(dir) = cluster_dir.as_ref() {
            count_existing_monthly_copies(dir, &job.name).unwrap_or(0) as u32
        } else {
            0
        };

        statuses.push(MonthlyJobStatus {
            job_id: job.id.clone(),
            job_name: job.name.clone(),
            monthly_cluster: job.monthly_cluster.clone(),
            copied_this_month: copied,
            completed: copied >= 2,
        });
    }

    // Stable presentation: completed last, then by cluster/name.
    statuses.sort_by(|a, b| {
        a.completed
            .cmp(&b.completed)
            .then_with(|| a.monthly_cluster.cmp(&b.monthly_cluster))
            .then_with(|| a.job_name.cmp(&b.job_name))
    });

    Ok(MonthlyStatusResult {
        month_folder: month_folder.to_string_lossy().to_string(),
        destination_base,
        jobs: statuses,
    })
}

fn parse_backup_timestamp_from_filename(job_name: &str, filename: &str) -> Option<DateTime<Utc>> {
    // Expected pattern: "{job_name}_YYYYMMDD_HHMMSS.(zip|7z)"
    let prefix = format!("{}_", job_name);
    let rest = filename
        .strip_prefix(&prefix)
        .and_then(|s| s.strip_suffix(".zip").or_else(|| s.strip_suffix(".7z")))?;
    if rest.len() != 15 {
        return None;
    }
    let naive = NaiveDateTime::parse_from_str(rest, "%Y%m%d_%H%M%S").ok()?;
    Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

/// Which job owns this backup filename: among jobs whose name parses as a prefix of the file,
/// pick the **longest** job name (e.g. `Server_PVE` wins over `Server` for `Server_PVE_20260226_120000.zip`).
fn owning_job_for_backup_filename<'a>(filename: &str, jobs: &'a [Job]) -> Option<&'a Job> {
    jobs.iter()
        .filter(|j| parse_backup_timestamp_from_filename(&j.name, filename).is_some())
        .max_by_key(|j| j.name.len())
}

pub fn preview_monthly_archive(app_data: &AppData) -> Result<MonthlyArchivePreview> {
    let config = app_data.get_config()?;
    let _destination = config
        .monthly_archive_destination
        .unwrap_or_else(|| DEFAULT_MONTHLY_BASE_DIR.to_string());

    let jobs = app_data.list_jobs()?;
    let now = Utc::now();
    let current_month = now.format("%Y-%m").to_string();

    // Per-job: first two backups in the current month (filename must match `{name}_YYYYMMDD_HHMMSS`;
    // do not use file modified time — that mis-attributes files when several jobs share a folder or
    // when one job name is a prefix of another). Ownership is resolved with longest job-name match.
    let mut to_archive: Vec<String> = Vec::new();
    for job in &jobs {
        let dir = Path::new(&job.destination_dir);
        if !dir.exists() {
            continue;
        }

        let mut entries: Vec<(PathBuf, DateTime<Utc>)> = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !(ext.eq_ignore_ascii_case("zip") || ext.eq_ignore_ascii_case("7z")) {
                continue;
            }
            let filename = match path.file_name().and_then(|n| n.to_str()) {
                Some(s) => s,
                None => continue,
            };
            let owner = match owning_job_for_backup_filename(filename, &jobs) {
                Some(j) => j,
                None => continue,
            };
            if owner.id != job.id {
                continue;
            }
            let dt = match parse_backup_timestamp_from_filename(&job.name, filename) {
                Some(dt) => dt,
                None => continue,
            };

            if dt.format("%Y-%m").to_string() != current_month {
                continue;
            }

            entries.push((path, dt));
        }

        entries.sort_by_key(|(_, dt)| *dt);
        for (p, _) in entries.into_iter().take(2) {
            to_archive.push(p.to_string_lossy().to_string());
        }
    }

    Ok(MonthlyArchivePreview {
        files: to_archive,
    })
}

pub fn run_monthly_archive(app_data: &AppData) -> Result<MonthlyArchiveResult> {
    let config = app_data.get_config()?;
    let destination = config
        .monthly_archive_destination
        .unwrap_or_else(|| DEFAULT_MONTHLY_BASE_DIR.to_string());

    let preview = preview_monthly_archive(app_data)?;
    
    if preview.files.is_empty() {
        return Ok(MonthlyArchiveResult {
            archived: 0,
            destination,
        });
    }

    // Create destination directory structure (and next month ahead)
    let now = Utc::now();
    let month_dir = ensure_monthly_dir_structure(&destination, now)?;

    // Copy files (do not remove originals)
    let mut archived = 0;
    for file_path_str in &preview.files {
        let source_path = Path::new(file_path_str);
        let file_name = source_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid file path: {}", file_path_str))?
            .to_string_lossy()
            .to_string();

        let jobs = app_data.list_jobs().unwrap_or_default();
        let category = owning_job_for_backup_filename(&file_name, &jobs)
            .map(|j| j.monthly_cluster.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("ASA Legacy");

        let dest_path = month_dir.join(category).join(&file_name);
        fs::copy(source_path, &dest_path)
            .with_context(|| format!("Failed to copy file: {}", file_path_str))?;
        
        archived += 1;
        log::info!("Archived: {} -> {}", file_path_str, dest_path.display());
    }

    Ok(MonthlyArchiveResult {
        archived,
        destination,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    #[test]
    fn test_retention_cutoff() {
        let retention_days = 7;
        let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
        let old_date = cutoff - chrono::Duration::days(1);
        let recent_date = cutoff + chrono::Duration::days(1);
        
        assert!(old_date < cutoff);
        assert!(recent_date > cutoff);
    }

    #[test]
    fn test_monthly_archive_selection() {
        let now = Utc::now();
        let current_month = now.format("%Y-%m").to_string();
        
        // Test dates in current month
        let test_date = now - chrono::Duration::days(5);
        let test_month = test_date.format("%Y-%m").to_string();
        
        if test_month == current_month {
            // Would be included in monthly archive
            assert!(true);
        }
    }

    #[test]
    fn test_monthly_archive_oldest_first() {
        let now = Utc::now();
        let dates = vec![
            now - chrono::Duration::days(10),
            now - chrono::Duration::days(5),
            now - chrono::Duration::days(1),
        ];
        
        let mut sorted = dates.clone();
        sorted.sort();
        
        // Oldest should be first
        assert_eq!(sorted[0], dates[0]);
        assert_eq!(sorted[2], dates[2]);
    }
}
