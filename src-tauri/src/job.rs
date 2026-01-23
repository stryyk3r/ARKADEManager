use crate::map::Map;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub root_dir: String,
    pub destination_dir: String,
    pub map: String, // Store as string for JSON compatibility
    pub include_saves: bool,
    pub include_map: bool,
    pub include_server_files: bool,
    pub include_plugin_configs: bool,
    pub interval_value: u32,
    pub interval_unit: String,
    pub retention_days: u32,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub last_file_size: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobInput {
    pub id: Option<String>,
    pub name: String,
    pub root_dir: String,
    pub destination_dir: String,
    pub map: String,
    pub include_saves: bool,
    pub include_map: bool,
    pub include_server_files: bool,
    pub include_plugin_configs: bool,
    pub interval_value: u32,
    pub interval_unit: String,
    pub retention_days: u32,
    pub enabled: bool,
}

impl Job {
    pub fn from_input(input: JobInput) -> Self {
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Utc::now();
        let next_run = if input.enabled {
            Some(calculate_next_run(now, input.interval_value, &input.interval_unit))
        } else {
            None
        };

        Self {
            id,
            name: input.name,
            root_dir: input.root_dir,
            destination_dir: input.destination_dir,
            map: input.map,
            include_saves: input.include_saves,
            include_map: input.include_map,
            include_server_files: input.include_server_files,
            include_plugin_configs: input.include_plugin_configs,
            interval_value: input.interval_value,
            interval_unit: input.interval_unit,
            retention_days: input.retention_days,
            enabled: input.enabled,
            last_run_at: None,
            next_run_at: next_run.map(|dt| dt.to_rfc3339()),
            last_file_size: None,
            last_error: None,
        }
    }

    pub fn get_map(&self) -> Option<Map> {
        Map::from_str(&self.map)
    }

    pub fn update_next_run(&mut self) {
        if self.enabled {
            let now = Utc::now();
            self.next_run_at = Some(
                calculate_next_run(now, self.interval_value, &self.interval_unit).to_rfc3339(),
            );
        }
    }

    pub fn update_after_run(&mut self, file_size: u64) {
        let now = Utc::now();
        self.last_run_at = Some(now.to_rfc3339());
        self.last_file_size = Some(file_size);
        self.last_error = None; // Clear any previous error on success
        self.update_next_run();
    }

    pub fn update_after_error(&mut self, error: String) {
        let now = Utc::now();
        self.last_run_at = Some(now.to_rfc3339());
        self.last_error = Some(error);
        self.update_next_run();
    }
}

fn calculate_next_run(now: DateTime<Utc>, value: u32, unit: &str) -> DateTime<Utc> {
    use chrono::Duration;
    let duration = match unit {
        "minutes" => Duration::minutes(value as i64),
        "hours" => Duration::hours(value as i64),
        "days" => Duration::days(value as i64),
        _ => Duration::hours(1),
    };
    now + duration
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_next_run() {
        let now = Utc::now();
        let next = calculate_next_run(now, 30, "minutes");
        assert!(next > now);
        let diff = next - now;
        assert!(diff.num_minutes() >= 29 && diff.num_minutes() <= 31);
        
        let next2 = calculate_next_run(now, 2, "hours");
        assert!(next2 > now);
        let diff2 = next2 - now;
        assert!(diff2.num_hours() >= 1 && diff2.num_hours() <= 3);
        
        let next3 = calculate_next_run(now, 1, "days");
        assert!(next3 > now);
        let diff3 = next3 - now;
        assert!(diff3.num_days() >= 0 && diff3.num_days() <= 2);
    }

    #[test]
    fn test_job_from_input() {
        let input = JobInput {
            id: None,
            name: "Test Job".to_string(),
            root_dir: r"C:\test".to_string(),
            destination_dir: r"C:\backups".to_string(),
            map: "TheIsland".to_string(),
            include_saves: true,
            include_map: true,
            include_server_files: false,
            include_plugin_configs: false,
            interval_value: 60,
            interval_unit: "minutes".to_string(),
            retention_days: 7,
            enabled: true,
        };

        let job = Job::from_input(input);
        assert_eq!(job.name, "Test Job");
        assert!(job.next_run_at.is_some());
        assert!(job.enabled);
    }

    #[test]
    fn test_job_update_after_run() {
        let mut input = JobInput {
            id: None,
            name: "Test".to_string(),
            root_dir: r"C:\test".to_string(),
            destination_dir: r"C:\backups".to_string(),
            map: "TheIsland".to_string(),
            include_saves: true,
            include_map: false,
            include_server_files: false,
            include_plugin_configs: false,
            interval_value: 1,
            interval_unit: "hours".to_string(),
            retention_days: 7,
            enabled: true,
        };

        let mut job = Job::from_input(input);
        let initial_next_run = job.next_run_at.clone();
        
        job.update_after_run(1024 * 1024); // 1MB
        
        assert!(job.last_run_at.is_some());
        assert_eq!(job.last_file_size, Some(1024 * 1024));
        assert!(job.next_run_at.is_some());
        assert_ne!(job.next_run_at, initial_next_run);
    }
}

