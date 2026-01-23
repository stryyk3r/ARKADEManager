use crate::config::Config;
use crate::job::{Job, JobInput};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde_json;
use std::fs;
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
        fs::write(&self.config_path, content)
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

    pub fn add_job(&mut self, job_input: JobInput) -> Result<Job> {
        let mut jobs = self.list_jobs().unwrap_or_default();
        let new_job = Job::from_input(job_input);
        jobs.push(new_job.clone());
        self.save_jobs(&jobs)?;
        Ok(new_job)
    }

    pub fn update_job(&mut self, job_input: JobInput) -> Result<Job> {
        let mut jobs = self.list_jobs().unwrap_or_default();
        let id = job_input.id.as_ref().ok_or_else(|| anyhow::anyhow!("Job ID required for update"))?;
        
        let job_index = jobs.iter().position(|j| j.id == *id)
            .ok_or_else(|| anyhow::anyhow!("Job not found"))?;
        
        let updated_job = Job::from_input(job_input);
        jobs[job_index] = updated_job.clone();
        self.save_jobs(&jobs)?;
        Ok(updated_job)
    }

    pub fn delete_job(&mut self, id: &str) -> Result<()> {
        let mut jobs = self.list_jobs().unwrap_or_default();
        jobs.retain(|j| j.id != id);
        self.save_jobs(&jobs)
    }

    pub fn save_jobs(&self, jobs: &[Job]) -> Result<()> {
        let content = serde_json::to_string_pretty(jobs)
            .context("Failed to serialize jobs")?;
        fs::write(&self.jobs_path, content)
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

