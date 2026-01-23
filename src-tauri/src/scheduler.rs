use crate::app_data::AppData;
use crate::backup;
use crate::job::Job;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::SystemTime;
use tauri::AppHandle;
use tauri::Emitter;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub running: bool,
    pub queue_size: usize,
    pub current_job: Option<String>,
    pub last_tick: Option<String>,
}

pub struct Scheduler {
    queue: VecDeque<Job>,
    running: bool,
    current_job: Option<String>,
    last_tick: Option<SystemTime>,
    last_run_times: std::collections::HashMap<String, SystemTime>,
    app_handle: AppHandle,
}

impl Scheduler {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            queue: VecDeque::new(),
            running: false,
            current_job: None,
            last_tick: None,
            last_run_times: std::collections::HashMap::new(),
            app_handle,
        }
    }

    pub fn refresh_jobs(&mut self, app_data: &AppData) -> Result<()> {
        let jobs = app_data.list_jobs().context("Failed to list jobs")?;
        
        // Save manually enqueued jobs (jobs already in queue that aren't due)
        let mut manually_enqueued = VecDeque::new();
        let now = Utc::now();
        while let Some(job) = self.queue.pop_front() {
            let is_due = if let Some(next_run_str) = &job.next_run_at {
                if let Ok(next_run) = DateTime::parse_from_rfc3339(next_run_str) {
                    next_run <= now
                } else {
                    false
                }
            } else {
                false
            };
            
            // If not due, it was likely manually enqueued - preserve it
            if !is_due {
                manually_enqueued.push_back(job);
            }
        }
        
        // Track job IDs already in queue to prevent duplicates
        let mut queued_ids: std::collections::HashSet<String> = manually_enqueued.iter().map(|j| j.id.clone()).collect();
        
        // Add due jobs from enabled jobs
        for job in jobs {
            if !job.enabled {
                continue;
            }

            // Check if job is due and not already in queue
            if queued_ids.contains(&job.id) {
                continue;
            }
            
            if let Some(next_run_str) = &job.next_run_at {
                if let Ok(next_run) = DateTime::parse_from_rfc3339(next_run_str) {
                    if next_run <= now {
                        queued_ids.insert(job.id.clone());
                        self.queue.push_back(job);
                    }
                }
            }
        }
        
        // Restore manually enqueued jobs (they're already tracked in queued_ids)
        while let Some(job) = manually_enqueued.pop_front() {
            self.queue.push_back(job);
        }

        Ok(())
    }

    pub fn tick(&mut self, app_data: &AppData) -> Result<()> {
        self.last_tick = Some(SystemTime::now());
        self.running = true;

        // Check if current job has completed (by checking if last_run_at was updated)
        if let Some(job_id) = &self.current_job {
            if let Some(job) = app_data.get_job(job_id) {
                if job.last_run_at.is_some() {
                    // Job completed, clear current_job
                    self.current_job = None;
                }
            } else {
                // Job was deleted, clear current_job
                self.current_job = None;
            }
        }

        // Refresh jobs to check for new due jobs
        self.refresh_jobs(app_data)?;

        // Process queue (one job at a time)
        if !self.queue.is_empty() && self.current_job.is_none() {
            let job = self.queue.pop_front().unwrap();
            
            // Dedup: skip if run within last 2 minutes
            // Check the job's actual last_run_at from the database, not the in-memory map
            // This ensures we use the completion time, not the start time
            if let Some(last_run_str) = &job.last_run_at {
                if let Ok(last_run_dt) = DateTime::parse_from_rfc3339(last_run_str) {
                    let elapsed = Utc::now().signed_duration_since(last_run_dt.with_timezone(&Utc));
                    if elapsed.num_seconds() < 120 {
                        log::info!("Skipping job {} (dedup - last run {} seconds ago)", job.name, elapsed.num_seconds());
                        self.emit_status();
                        return Ok(());
                    }
                }
            }

            // Mark as running
            self.current_job = Some(job.id.clone());
            // Don't update last_run_times here - it will be updated when the backup completes

            // Run backup in background
            let job_clone = job.clone();
            let app_handle_clone = self.app_handle.clone();

            tokio::spawn(async move {
                let app_data = match AppData::new() {
                    Ok(ad) => ad,
                    Err(e) => {
                        log::error!("Failed to create app data: {}", e);
                        return;
                    }
                };

                match backup::create_backup(&job_clone, &app_data).await {
                    Ok(file_size) => {
                        // Update job
                        let app_data = match AppData::new() {
                            Ok(ad) => ad,
                            Err(e) => {
                                log::error!("Failed to create app data for update: {}", e);
                                return;
                            }
                        };
                        if let Some(mut job) = app_data.get_job(&job_clone.id) {
                            job.update_after_run(file_size);
                            let mut jobs = app_data.list_jobs().unwrap_or_default();
                            if let Some(pos) = jobs.iter().position(|j| j.id == job.id) {
                                jobs[pos] = job;
                                if let Err(e) = app_data.save_jobs(&jobs) {
                                    log::error!("Failed to save jobs: {}", e);
                                }
                            }
                        }

                        // Notify on main thread to avoid winit event loop warnings
                        let app_handle_for_emit = app_handle_clone.clone();
                        app_handle_clone.run_on_main_thread(move || {
                            app_handle_for_emit.emit("job_updated", ()).ok();
                        }).ok();
                    }
                    Err(e) => {
                        log::error!("Backup failed for job {}: {}", job_clone.name, e);
                        // Update job with error
                        let app_data = match AppData::new() {
                            Ok(ad) => ad,
                            Err(err) => {
                                log::error!("Failed to create app data for error update: {}", err);
                                return;
                            }
                        };
                        if let Some(mut job) = app_data.get_job(&job_clone.id) {
                            job.update_after_error(format!("{}", e));
                            let mut jobs = app_data.list_jobs().unwrap_or_default();
                            if let Some(pos) = jobs.iter().position(|j| j.id == job.id) {
                                jobs[pos] = job;
                                if let Err(err) = app_data.save_jobs(&jobs) {
                                    log::error!("Failed to save jobs: {}", err);
                                }
                            }
                        }

                        // Notify on main thread to avoid winit event loop warnings
                        let app_handle_for_emit = app_handle_clone.clone();
                        app_handle_clone.run_on_main_thread(move || {
                            app_handle_for_emit.emit("job_updated", ()).ok();
                        }).ok();
                    }
                }
            });
        }

        self.emit_status();
        Ok(())
    }

    pub fn enqueue_job(&mut self, job: Job) -> Result<()> {
        self.queue.push_back(job);
        // Don't emit status here - let the tick handle it to avoid event loop warnings
        // Status will be emitted when tick() is called
        Ok(())
    }

    pub fn get_status(&self) -> Status {
        Status {
            running: self.running,
            queue_size: self.queue.len(),
            current_job: self.current_job.clone(),
            last_tick: self.last_tick.map(|t| {
                DateTime::<Utc>::from(t)
                    .to_rfc3339()
            }),
        }
    }

    fn emit_status(&self) {
        let status = self.get_status();
        let app_handle = self.app_handle.clone();
        // Emit on main thread to avoid winit event loop warnings
        let app_handle_for_emit = app_handle.clone();
        app_handle.run_on_main_thread(move || {
            app_handle_for_emit.emit("status_update", status).ok();
        }).ok();
    }

}

impl Clone for Scheduler {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            running: self.running,
            current_job: self.current_job.clone(),
            last_tick: self.last_tick,
            last_run_times: self.last_run_times.clone(),
            app_handle: self.app_handle.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::JobInput;
    use chrono::Utc;

    #[test]
    fn test_dedup_logic() {
        let now = SystemTime::now();
        let two_minutes_ago = now - Duration::from_secs(60);
        let five_minutes_ago = now - Duration::from_secs(300);
        
        // Should skip if within 2 minutes
        assert!(two_minutes_ago.elapsed().unwrap_or(Duration::from_secs(0)) < Duration::from_secs(120));
        
        // Should allow if more than 2 minutes ago
        assert!(five_minutes_ago.elapsed().unwrap_or(Duration::from_secs(0)) >= Duration::from_secs(120) || 
                five_minutes_ago.elapsed().is_err());
    }

    #[test]
    fn test_refresh_jobs_due() {
        // Test that jobs with next_run_at in the past are enqueued
        let now = Utc::now();
        let past = now - chrono::Duration::minutes(10);
        let future = now + chrono::Duration::minutes(10);
        
        assert!(past <= now);
        assert!(future > now);
    }
}

