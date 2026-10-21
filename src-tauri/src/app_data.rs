use crate::config::Config;
use crate::job::{Job, JobInput};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde_json;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "config.json";
const JOBS_FILE: &str = "backup_jobs.json";
const LOGS_DIR: &str = "logs";

pub struct AppData {
    _data_dir: PathBuf,
    config_path: PathBuf,
    jobs_path: PathBuf,
    logs_dir: PathBuf,
}

impl AppData {
    pub fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "arkade", "manager")
            .context("Failed to get project directories")?;
        let data_dir = proj_dirs.data_dir().to_path_buf();

        // Create directories if they don't exist
        fs::create_dir_all(&data_dir)
            .context("Failed to create data directory")?;

        let logs_dir = data_dir.join(LOGS_DIR);
        fs::create_dir_all(&logs_dir)
            .context("Failed to create logs directory")?;

        let config_path = data_dir.join(CONFIG_FILE);
        let jobs_path = data_dir.join(JOBS_FILE);

        let mut app_data = Self {
            _data_dir: data_dir,
            config_path,
            jobs_path,
            logs_dir,
        };

        // Initialize config if it doesn't exist
        if !app_data.config_path.exists() {
            let default_config = Config::default();
            app_data.save_config(&default_config)?;
        }

        // Initialize jobs file if it doesn't exist
        if !app_data.jobs_path.exists() {
            app_data.save_jobs(&Vec::new())?;
        }

        Ok(app_data)
    }

    pub fn get_config(&self) -> Result<Config> {
        let content = fs::read_to_string(&self.config_path)
            .context("Failed to read config file")?;
        let config: Config = serde_json::from_str(&content)
            .context("Failed to parse config file")?;
        Ok(config)
    }

    pub fn save_config(&mut self, config: &Config) -> Result<()> {
        let content = serde_json::to_string_pretty(config)
            .context("Failed to serialize config")?;
        write_atomic(&self.config_path, content.as_bytes())
            .context("Failed to write config file")?;
        Ok(())
    }

    pub fn list_jobs(&self) -> Result<Vec<Job>> {
        let content = fs::read_to_string(&self.jobs_path)
            .context("Failed to read jobs file")?;
        let jobs: Vec<Job> = serde_json::from_str(&content)
            .context("Failed to parse jobs file")?;
        Ok(jobs)
    }

    pub fn get_job(&self, id: &str) -> Option<Job> {
        let jobs = self.list_jobs().ok()?;
        jobs.into_iter().find(|j| j.id == id)
    }

    // add/update/delete must propagate read errors: falling back to an empty list here would
    // overwrite backup_jobs.json and wipe every other job (e.g. when antivirus briefly locks the file).
    pub fn add_job(&mut self, job_input: JobInput) -> Result<Job> {
        let mut jobs = self.list_jobs()?;
        let new_job = Job::from_input(job_input);
        jobs.push(new_job.clone());
        self.save_jobs(&jobs)?;
        Ok(new_job)
    }

    pub fn update_job(&mut self, job_input: JobInput) -> Result<Job> {
        let mut jobs = self.list_jobs()?;
        let id = job_input.id.as_ref().ok_or_else(|| anyhow::anyhow!("Job ID required for update"))?;
        
        let job_index = jobs.iter().position(|j| j.id == *id)
            .ok_or_else(|| anyhow::anyhow!("Job not found"))?;
        
        let existing = jobs[job_index].clone();
        let updated_job = Job::from_input_with_existing(job_input, Some(existing));
        jobs[job_index] = updated_job.clone();
        self.save_jobs(&jobs)?;
        Ok(updated_job)
    }

    pub fn delete_job(&mut self, id: &str) -> Result<()> {
        let mut jobs = self.list_jobs()?;
        jobs.retain(|j| j.id != id);
        self.save_jobs(&jobs)
    }

    pub fn save_jobs(&self, jobs: &[Job]) -> Result<()> {
        let content = serde_json::to_string_pretty(jobs)
            .context("Failed to serialize jobs")?;
        write_atomic(&self.jobs_path, content.as_bytes())
            .context("Failed to write jobs file")?;
        Ok(())
    }

    pub fn get_logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    #[allow(dead_code)]
    pub fn get_data_dir(&self) -> &Path {
        &self._data_dir
    }
}

const REPLACE_RETRY_ATTEMPTS: u32 = 5;
const REPLACE_RETRY_DELAY_MS: u64 = 100;

/// Write a file so readers only ever see the old or the new contents, never a truncated file:
/// write a uniquely named temp file next to it, flush it to disk, then rename it over the target.
/// The rename is retried briefly because antivirus/indexers can hold the target open on Windows.
fn write_atomic(path: &Path, content: &[u8]) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid file path")?;
    let temp_path = path.with_file_name(format!("{}.{}.tmp", file_name, uuid::Uuid::new_v4()));

    let write_result = (|| -> Result<()> {
        let mut file = fs::File::create(&temp_path)
            .with_context(|| format!("Failed to create temp file: {}", temp_path.display()))?;
        file.write_all(content)
            .with_context(|| format!("Failed to write temp file: {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("Failed to sync temp file: {}", temp_path.display()))?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(e);
    }

    let mut attempt = 1;
    loop {
        match fs::rename(&temp_path, path) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < REPLACE_RETRY_ATTEMPTS => {
                log::warn!(
                    "Failed to replace {} (attempt {}/{}): {}. Retrying...",
                    path.display(),
                    attempt,
                    REPLACE_RETRY_ATTEMPTS,
                    e
                );
                std::thread::sleep(std::time::Duration::from_millis(REPLACE_RETRY_DELAY_MS));
                attempt += 1;
            }
            Err(e) => {
                let _ = fs::remove_file(&temp_path);
                return Err(e).with_context(|| format!("Failed to replace {}", path.display()));
            }
        }
    }
}

pub fn read_logs(lines: usize) -> Result<String> {
    let app_data = AppData::new()?;
    let logs_dir = app_data.get_logs_dir();
    
    // Get all log files, sorted by modification time
    let mut log_files: Vec<_> = fs::read_dir(logs_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "log")
                .unwrap_or(false)
        })
        .collect();
    
    log_files.sort_by_key(|e| {
        e.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    
    // Read from most recent log file
    if let Some(most_recent) = log_files.last() {
        let content = fs::read_to_string(most_recent.path())?;
        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(lines);
        Ok(all_lines[start..].join("\n"))
    } else {
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_data_in(dir: &Path) -> AppData {
        AppData {
            _data_dir: dir.to_path_buf(),
            config_path: dir.join(CONFIG_FILE),
            jobs_path: dir.join(JOBS_FILE),
            logs_dir: dir.join(LOGS_DIR),
        }
    }

    #[test]
    fn write_atomic_replaces_contents_and_leaves_no_temp_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("data.json");
        fs::write(&path, "old").unwrap();

        write_atomic(&path, b"new").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let entries: Vec<_> = fs::read_dir(temp.path()).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn unreadable_jobs_file_is_not_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let mut app_data = app_data_in(temp.path());
        let corrupt = "[{\"id\": \"truncated";
        fs::write(&app_data.jobs_path, corrupt).unwrap();

        assert!(app_data.delete_job("anything").is_err());
        assert_eq!(fs::read_to_string(&app_data.jobs_path).unwrap(), corrupt);
    }
}

