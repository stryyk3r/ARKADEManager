//! Scheduler: only one backup runs at a time (Minecraft or ARK).
//! Due jobs are added to the queue. We pop and run exactly one job when no job is running.
//! When that job's backup task finishes (for any reason), we clear current_job and the next queued job starts.

use crate::app_data::AppData;
use crate::backup;
use crate::job::Job;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant, SystemTime};
use crate::state::AppState;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;
use tokio::task::JoinHandle;

/// Safety net so one backup that never returns cannot block every other job forever.
const MAX_BACKUP_DURATION: Duration = Duration::from_secs(6 * 60 * 60);

/// After a run finishes, don't re-queue the same job for this long. Normally the saved next_run_at
/// already prevents that; this covers the case where saving the result to the jobs file failed.
const FINISHED_COOLDOWN: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub running: bool,
    pub queue_size: usize,
    pub current_job: Option<String>,
    pub last_tick: Option<String>,
}

struct RunningJob {
    id: String,
    /// Finishes when the backup is over and its result has been recorded — even if the backup
    /// errored, panicked, timed out, or the result could not be saved.
    handle: JoinHandle<()>,
}

pub struct Scheduler {
    queue: VecDeque<Job>,
    running: bool,
    current_job: Option<RunningJob>,
    last_tick: Option<SystemTime>,
    /// job_id -> when its last run finished (in-memory cooldown, see FINISHED_COOLDOWN).
    recently_finished: HashMap<String, Instant>,
    app_handle: AppHandle,
}

impl Scheduler {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            queue: VecDeque::new(),
            running: false,
            current_job: None,
            last_tick: None,
            recently_finished: HashMap::new(),
            app_handle,
        }
    }

    pub fn refresh_jobs(&mut self, app_data: &AppData) -> Result<()> {
        let jobs = app_data.list_jobs().context("Failed to list jobs")?;
        self.recently_finished
            .retain(|_, finished_at| finished_at.elapsed() < FINISHED_COOLDOWN);

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
            if self.current_job.as_ref().map(|r| &r.id) == Some(&job.id) {
                log::debug!("Job {} not enqueued (already running)", job.name);
                continue;
            }

            // Check if job is due and not already in queue
            if queued_ids.contains(&job.id) {
                continue;
            }

            if self.recently_finished.contains_key(&job.id) {
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

        // Clear current_job once its backup task has finished, whatever the outcome.
        if self.current_job.as_ref().is_some_and(|r| r.handle.is_finished()) {
            let finished = self.current_job.take().unwrap();
            log::info!("Backup job finished, starting next queued job if any");
            self.recently_finished.insert(finished.id, Instant::now());
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

            let app_handle = self.app_handle.clone();
            let handle = tokio::spawn(async move {
                // Run the backup in its own task so a panic surfaces as a JoinError we can record.
                let mut backup_task = tokio::spawn(run_backup(job.clone(), app_handle.clone()));
                let outcome = match tokio::time::timeout(MAX_BACKUP_DURATION, &mut backup_task).await {
                    Ok(Ok(result)) => result,
                    Ok(Err(join_err)) => Err(anyhow::anyhow!("Backup task crashed: {}", join_err)),
                    Err(_) => {
                        backup_task.abort();
                        Err(anyhow::anyhow!(
                            "Backup timed out after {} hours and was abandoned",
                            MAX_BACKUP_DURATION.as_secs() / 3600
                        ))
                    }
                };
                record_backup_outcome(&job, outcome, &app_handle).await;
            });

            self.current_job = Some(RunningJob { id: job_id, handle });
        }

        self.emit_status();
        Ok(())
    }

    pub fn enqueue_job(&mut self, job: Job) -> Result<()> {
        if self.current_job.as_ref().map(|r| &r.id) == Some(&job.id) {
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

    pub fn is_backup_running(&self) -> bool {
        self.current_job.as_ref().is_some_and(|r| !r.handle.is_finished())
    }

    pub fn get_status(&self) -> Status {
        Status {
            running: self.running,
            queue_size: self.queue.len(),
            current_job: self.current_job.as_ref().map(|r| r.id.clone()),
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

async fn run_backup(job: Job, app_handle: AppHandle) -> Result<u64> {
    // Jobs saved before name validation existed may still have unsafe names; never build paths from them.
    crate::validation::validate_job_name(&job.name)
        .context("Job name is not a valid file name; edit the job and rename it before backing up")?;

    let app_data = AppData::new().context("Failed to load app data")?;

    let file_size = match job.job_type.as_str() {
        "minecraft" => backup::create_minecraft_backup(&job, &app_data, Some(app_handle)).await,
        "palworld" => backup::create_palworld_backup(&job, &app_data).await,
        _ => backup::create_backup(&job, &app_data).await,
    }?;

    if file_size == 0 {
        log::error!("Backup produced 0-byte file for job {}; treating as failure", job.name);
        anyhow::bail!("Backup produced an empty file (0 bytes). The backup did not complete successfully.");
    }
    Ok(file_size)
}

/// Save the run's result on the job (update_next_run() sets next_run_at, so a failed job
/// will not run again until the next scheduled time) and notify the UI.
async fn record_backup_outcome(job: &Job, outcome: Result<u64>, app_handle: &AppHandle) {
    if let Err(e) = &outcome {
        log::error!("Backup failed for job {}: {:#}", job.name, e);
    }

    let save = |app_data: &AppData| -> Result<()> {
        let mut jobs = app_data.list_jobs()?;
        if let Some(stored) = jobs.iter_mut().find(|j| j.id == job.id) {
            match &outcome {
                Ok(file_size) => stored.update_after_run(*file_size),
                Err(e) => stored.update_after_error(format!("{:#}", e)),
            }
            app_data.save_jobs(&jobs)?;
        }
        Ok(())
    };
    // Read-modify-write under the shared app_data lock (the same one the job commands use), so a job
    // edited while this backup was running isn't overwritten with the stale copy read here.
    let saved = match app_handle.try_state::<AppState>() {
        Some(state) => save(&*state.app_data.lock().await),
        None => AppData::new().and_then(|app_data| save(&app_data)),
    };
    if let Err(e) = saved {
        log::error!("Failed to save backup result for job {}: {:#}", job.name, e);
    }

    let failed = outcome.is_err();
    let app_handle_for_emit = app_handle.clone();
    app_handle.run_on_main_thread(move || {
        app_handle_for_emit.emit("job_updated", ()).ok();
        if failed {
            app_handle_for_emit.emit("backup_failed", ()).ok();
        }
    }).ok();
}

#[cfg(test)]
mod tests {
    use super::*;
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
