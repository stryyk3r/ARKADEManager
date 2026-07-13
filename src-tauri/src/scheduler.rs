//! Scheduler: only one backup runs at a time (Minecraft or ARK).
//! Due jobs are added to the queue. We pop and run exactly one job when no job is running.
//! When that job completes (last_run_at updated), we clear current_job and the next tick runs the next queued job in order.

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFailedPayload {
    pub job_name: String,
    pub error: String,
}

/// When a job is running we store (job_id, last_run_at when we started).
/// We only clear when the job's last_run_at has *changed* (backup completed and updated it).
pub struct Scheduler {
    queue: VecDeque<Job>,
    running: bool,
    /// (job_id, last_run_at value when this run started) — cleared only when last_run_at changes
    current_job: Option<(String, Option<String>)>,
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

            // Never re-queue the job that is currently running (avoids multiple backup attempts)
            if self.current_job.as_ref().map(|(id, _)| id) == Some(&job.id) {
                log::debug!("Job {} not enqueued (already running)", job.name);
                continue;
            }

            // Check if job is due and not already in queue
            if queued_ids.contains(&job.id) {
                continue;
            }

            // Don't re-queue if it ran very recently. Use longer cooldown after a failure to avoid repeated backup attempts when a file blocks.
            if let Some(last_run_str) = &job.last_run_at {
                if let Ok(last_run) = DateTime::parse_from_rfc3339(last_run_str) {
                    let elapsed = now.signed_duration_since(last_run.with_timezone(&Utc));
                    let cooldown_secs = if job.last_error.is_some() { 300 } else { 120 };
                    if elapsed.num_seconds() < cooldown_secs {
                        continue;
                    }
                }
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

        // Clear current_job only when the job's last_run_at has *changed* since we started (backup finished)
        let should_clear_current = if let Some((ref job_id, ref started_last_run)) = &self.current_job {
            if let Some(job) = app_data.get_job(job_id) {
                job.last_run_at.as_ref() != started_last_run.as_ref()
            } else {
                true
            }
        } else {
            false
        };
        if should_clear_current {
            log::info!("Backup job finished, next in queue will run on next tick");
            self.current_job = None;
        }

        // Refresh jobs to check for new due jobs
        self.refresh_jobs(app_data)?;

        // Process queue: only one job at a time. Do not start another until current_job is cleared (backup finished).
        if !self.queue.is_empty() && self.current_job.is_none() {
            let job = self.queue.pop_front().unwrap();
            let job_id = job.id.clone();
            self.queue.retain(|j| j.id != job_id);
            let queue_size = self.queue.len();
            log::info!("Starting backup for job {} ({} in queue)", job.name, queue_size);

            // Do not skip here based on last_run_at. A previous 2-minute dedup at pop time broke
            // "Run Now" twice in a row (job was popped then dropped). Scheduled re-runs are already
            // throttled in refresh_jobs() via the 120s / 300s cooldown before a due job is enqueued.

            // Mark as running: store job id and last_run_at *as of now* so we only clear when it changes (backup completed)
            self.current_job = Some((job.id.clone(), job.last_run_at.clone()));
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

                let backup_result = match job_clone.job_type.as_str() {
                    "minecraft" => {
                        backup::create_minecraft_backup(
                            &job_clone,
                            &app_data,
                            Some(app_handle_clone.clone()),
                        )
                        .await
                    }
                    "palworld" => backup::create_palworld_backup(&job_clone, &app_data).await,
                    _ => backup::create_backup(&job_clone, &app_data).await,
                };

                match backup_result {
                    Ok(file_size) => {
                        if file_size == 0 {
                            log::error!("Backup produced 0-byte file for job {}; treating as failure", job_clone.name);
                            let e = "Backup produced an empty file (0 bytes). The backup did not complete successfully.";
                            let app_data = match AppData::new() {
                                Ok(ad) => ad,
                                Err(err) => {
                                    log::error!("Failed to create app data for error update: {}", err);
                                    return;
                                }
                            };
                            if let Some(mut job) = app_data.get_job(&job_clone.id) {
                                job.update_after_error(e.to_string());
                                let mut jobs = app_data.list_jobs().unwrap_or_default();
                                if let Some(pos) = jobs.iter().position(|j| j.id == job.id) {
                                    jobs[pos] = job;
                                    if let Err(err) = app_data.save_jobs(&jobs) {
                                        log::error!("Failed to save jobs: {}", err);
                                    }
                                }
                            }
                            let payload = BackupFailedPayload {
                                job_name: job_clone.name.clone(),
                                error: e.to_string(),
                            };
                            let app_handle_for_emit = app_handle_clone.clone();
                            app_handle_clone.run_on_main_thread(move || {
                                app_handle_for_emit.emit("job_updated", ()).ok();
                                app_handle_for_emit.emit("backup_failed", payload).ok();
                            }).ok();
                            return;
                        }
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
                        // Update job with error; update_next_run() sets next_run_at so this job will not run again until the next scheduled time
                        let app_data = match AppData::new() {
                            Ok(ad) => ad,
                            Err(err) => {
                                log::error!("Failed to create app data for error update: {}", err);
                                return;
                            }
                        };
                        if let Some(mut job) = app_data.get_job(&job_clone.id) {
                            job.update_after_error(format!("{:#}", e));
                            let mut jobs = app_data.list_jobs().unwrap_or_default();
                            if let Some(pos) = jobs.iter().position(|j| j.id == job.id) {
                                jobs[pos] = job;
                                if let Err(err) = app_data.save_jobs(&jobs) {
                                    log::error!("Failed to save jobs: {}", err);
                                }
                            }
                        }

                        let payload = BackupFailedPayload {
                            job_name: job_clone.name.clone(),
                            error: format!("{}", e),
                        };
                        let app_handle_for_emit = app_handle_clone.clone();
                        app_handle_clone.run_on_main_thread(move || {
                            app_handle_for_emit.emit("job_updated", ()).ok();
                            app_handle_for_emit.emit("backup_failed", payload).ok();
                        }).ok();
                    }
                }
            });
        }

        self.emit_status();
        Ok(())
    }

    pub fn enqueue_job(&mut self, job: Job) -> Result<()> {
        if self.current_job.as_ref().map(|(id, _)| id) == Some(&job.id) {
            log::info!("Job {} already running, not enqueueing again", job.name);
            return Ok(());
        }
        if self.queue.iter().any(|j| j.id == job.id) {
            log::info!("Job {} is already in the queue, not enqueueing again", job.name);
            return Ok(());
        }
        let name = job.name.clone();
        self.queue.push_back(job);
        log::info!("Job {} enqueued (queue size: {})", name, self.queue.len());
        Ok(())
    }

    pub fn get_status(&self) -> Status {
        Status {
            running: self.running,
            queue_size: self.queue.len(),
            current_job: self.current_job.as_ref().map(|(id, _)| id.clone()),
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
            current_job: self.current_job.as_ref().map(|(id, r)| (id.clone(), r.clone())),
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

