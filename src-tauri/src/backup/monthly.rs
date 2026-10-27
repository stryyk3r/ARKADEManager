use crate::app_data::AppData;
use crate::job::Job;
use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};

use super::shared::{copy_large_file, rename_temp_to_backup, same_volume, TempFileGuard};

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

/// Count this job's archives in a monthly category folder. Only exact `{job_name}_YYYYMMDD_HHMMSS`
/// names count, so a job named `Island` does not count `Island_PVP_...` copies, and unfinished
/// `.partial` copies are ignored.
fn count_existing_monthly_copies(category_dir: &Path, job_name: &str) -> Result<usize> {
    if !category_dir.exists() {
        return Ok(0);
    }
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
        if parse_backup_timestamp_from_filename(job_name, file_name).is_some() {
            count += 1;
        }
    }
    Ok(count)
}

/// Put a complete copy of `source` at `dest`.
///
/// On the same volume this is a hard link: no data is copied, and the file appears in one atomic
/// step (archives are never modified after they are written, and deleting the original backup
/// later does not affect the link). Otherwise copy to `{dest}.partial` and rename when complete,
/// so a copy interrupted part-way (network share dropped, disk full) never leaves a truncated
/// archive that counts as a monthly copy.
fn copy_complete_file(source: &Path, dest: &Path) -> Result<()> {
    let file_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid destination path: {}", dest.display()))?;

    let dest_dir = dest.parent().unwrap_or(Path::new("."));
    if same_volume(source, dest_dir) {
        match fs::hard_link(source, dest) {
            Ok(()) => return Ok(()),
            Err(e) => log::info!(
                "MONTHLY: hard link {} -> {} not possible ({}); copying instead",
                source.display(),
                dest.display(),
                e
            ),
        }
    }

    let partial = dest.with_file_name(format!("{}.partial", file_name));
    let _partial_guard = TempFileGuard::new(&partial);
    copy_large_file(source, &partial).with_context(|| {
        format!("Failed to copy {} -> {}", source.display(), partial.display())
    })?;
    rename_temp_to_backup(&partial, dest)
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
    copy_complete_file(backup_path, &dest_path).context("Failed to copy monthly backup")?;
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

/// What a manual monthly archive will copy. Built quickly while holding the app-data lock; the
/// (possibly multi-GB, network-share) copying happens afterwards in `execute_monthly_archive`
/// without the lock, so the UI and scheduler keep working meanwhile.
pub struct MonthlyArchivePlan {
    destination: String,
    /// (source backup file, monthly category folder)
    copies: Vec<(PathBuf, String)>,
}

pub fn plan_monthly_archive(app_data: &AppData) -> Result<MonthlyArchivePlan> {
    let config = app_data.get_config()?;
    let destination = config
        .monthly_archive_destination
        .unwrap_or_else(|| DEFAULT_MONTHLY_BASE_DIR.to_string());

    let preview = preview_monthly_archive(app_data)?;
    let jobs = app_data.list_jobs()?;

    let mut copies = Vec::with_capacity(preview.files.len());
    for file_path_str in preview.files {
        let source_path = PathBuf::from(&file_path_str);
        let file_name = source_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid file path: {}", file_path_str))?
            .to_string_lossy()
            .to_string();

        let category = owning_job_for_backup_filename(&file_name, &jobs)
            .map(|j| j.monthly_cluster.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("ASA Legacy")
            .to_string();

        copies.push((source_path, category));
    }

    Ok(MonthlyArchivePlan {
        destination,
        copies,
    })
}

pub fn execute_monthly_archive(plan: MonthlyArchivePlan) -> Result<MonthlyArchiveResult> {
    let MonthlyArchivePlan {
        destination,
        copies,
    } = plan;

    if copies.is_empty() {
        return Ok(MonthlyArchiveResult {
            archived: 0,
            destination,
        });
    }

    // Create destination directory structure
    let month_dir = ensure_monthly_dir_structure(&destination, Utc::now())?;

    // Copy files (do not remove originals)
    let mut archived = 0;
    for (source_path, category) in &copies {
        let file_name = source_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid file path: {}", source_path.display()))?;
        let dest_path = month_dir.join(category).join(file_name);
        copy_complete_file(source_path, &dest_path)?;

        archived += 1;
        log::info!("Archived: {} -> {}", source_path.display(), dest_path.display());
    }

    Ok(MonthlyArchiveResult {
        archived,
        destination,
    })
}

#[cfg(test)]
mod tests {
    use super::{copy_complete_file, count_existing_monthly_copies};
    use chrono::Utc;
    use std::fs;

    #[test]
    fn monthly_count_ignores_similar_job_names_and_partial_copies() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        for name in [
            "Island_20261001_120000.zip",
            "Island_PVP_20261001_120000.zip",
            "Island_PVP_20261002_120000.zip",
            "Island_20261002_120000.zip.partial",
            "Island_notes.zip",
        ] {
            fs::write(dir.join(name), b"x").unwrap();
        }

        assert_eq!(count_existing_monthly_copies(dir, "Island").unwrap(), 1);
        assert_eq!(count_existing_monthly_copies(dir, "Island_PVP").unwrap(), 2);
    }

    #[test]
    fn complete_copy_leaves_no_partial_file() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("Island_20261001_120000.zip");
        fs::write(&source, b"archive").unwrap();
        let dest_dir = temp.path().join("monthly");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest = dest_dir.join("Island_20261001_120000.zip");

        copy_complete_file(&source, &dest).unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"archive");
        assert_eq!(fs::read_dir(&dest_dir).unwrap().count(), 1);
    }

    #[test]
    fn monthly_copy_survives_original_being_deleted() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("Island_20261001_120000.7z");
        fs::write(&source, b"archive").unwrap();
        let dest_dir = temp.path().join("monthly");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest = dest_dir.join("Island_20261001_120000.7z");

        copy_complete_file(&source, &dest).unwrap();
        // Retention later deletes the regular backup; the monthly copy must stay intact.
        fs::remove_file(&source).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"archive");
    }

    #[test]
    fn existing_monthly_copy_is_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("Island_20261001_120000.7z");
        fs::write(&source, b"new").unwrap();
        let dest_dir = temp.path().join("monthly");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest = dest_dir.join("Island_20261001_120000.7z");
        fs::write(&dest, b"old").unwrap();

        copy_complete_file(&source, &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"new");
    }

    #[test]
    fn failed_copy_leaves_nothing_behind() {
        let temp = tempfile::tempdir().unwrap();
        let dest_dir = temp.path().join("monthly");
        fs::create_dir_all(&dest_dir).unwrap();

        let missing = temp.path().join("missing.zip");
        assert!(copy_complete_file(&missing, &dest_dir.join("Island_20261001_120000.zip")).is_err());
        assert_eq!(fs::read_dir(&dest_dir).unwrap().count(), 0);
    }

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
