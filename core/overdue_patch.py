# core/overdue_patch.py

import os
import glob
from datetime import datetime, timedelta

def _job_output_dir(self, job: dict) -> str:
    """
    Return the folder where this job writes backup zips.
    Adjust keys if your job schema differs.
    """
    # First check for destination directory (which is the correct field for this app)
    out = job.get("destination")
    if out:
        if hasattr(self, "logger"):
            self.logger.info(f"Using destination from job: {out}")
        return out
    
    # Fallback to other possible field names
    out = job.get("save_directory") or job.get("output_dir")
    if out:
        if hasattr(self, "logger"):
            self.logger.info(f"Using fallback destination: {out}")
        return out
    
    # If no valid destination found, create a default one
    zip_name = job.get("zip_name", "backup")
    default = os.path.join(self.SCRIPT_DIR if hasattr(self, "SCRIPT_DIR") else os.getcwd(),
                           "backups", zip_name)
    os.makedirs(default, exist_ok=True)
    if hasattr(self, "logger"):
        self.logger.info(f"Using default destination: {default}")
    return default

def _job_zip_glob(self, job: dict):
    """
    Patterns to match backup zips for a job. Adjust if your naming differs.
    """
    zip_name = job.get("zip_name", "backup")
    out_dir = _job_output_dir(self, job)
    patterns = [
        os.path.join(out_dir, f"{zip_name}_*.zip"),  # e.g., backup_2025-08-26_132100.zip
        os.path.join(out_dir, f"{zip_name}.zip"),
        os.path.join(out_dir, "*.zip"),
    ]
    
    # Debug logging
    # if hasattr(self, "logger"):
    #     self.logger.info(f"Job {zip_name}: Created search patterns: {patterns}")
    
    return patterns, out_dir

def get_latest_backup(self, job: dict):
    """
    Returns (path, mtime_dt, size_bytes) for the newest .zip backup,
    or (None, None, None) if none exist.
    """
    try:
        # Get the destination directory for this job
        out_dir = job.get("destination")
        if not out_dir:
            if hasattr(self, "logger"):
                self.logger.warning(f"No destination directory for job {job.get('zip_name', 'unknown')}")
            return None, None, None
        
        # Check if the directory exists
        if not os.path.exists(out_dir):
            if hasattr(self, "logger"):
                self.logger.info(f"Destination directory does not exist: {out_dir}")
            return None, None, None
        
        # Find all .zip files in the destination directory
        zip_pattern = os.path.join(out_dir, "*.zip")
        zip_files = glob.glob(zip_pattern)
        
        if not zip_files:
            if hasattr(self, "logger"):
                self.logger.info(f"No zip files found in {out_dir}")
            return None, None, None
        
        # Get the most recent zip file by modification time
        latest_zip = max(zip_files, key=os.path.getmtime)
        st = os.stat(latest_zip)
        
        # Debug logging removed as requested
        
        return latest_zip, datetime.fromtimestamp(st.st_mtime), st.st_size
    except Exception as e:
        if hasattr(self, "logger"):
            self.logger.error(f"Error in get_latest_backup for job {job.get('zip_name', 'unknown')}: {e}")
        return None, None, None

def _interval_seconds(self, job: dict) -> int:
    """
    Compute a job's interval in seconds from typical keys.
    Adjust keys to your schema if needed.
    """
    value = int(job.get("interval_value", 0) or 0)
    unit  = str(job.get("interval_unit", "minutes")).lower()
    if unit.startswith("sec"):  return max(1, value)
    if unit.startswith("min"):  return max(60, value * 60)
    if unit.startswith("hour"): return max(3600, value * 3600)
    if unit.startswith("day"):  return max(86400, value * 86400)
    # fallback: minutes
    return max(60, value * 60)

def ensure_overdue_jobs_run(self):
    """
    If (now - last_backup_time) >= interval, run immediately, and set next_run_at.
    Only runs jobs that are significantly overdue (more than 5 minutes past due).
    """
    now = datetime.now()
    changed = False

    jobs = list(getattr(self, "active_jobs", []))
    for job in jobs:
        try:
            interval = _interval_seconds(self, job)
            _, last_dt, _ = get_latest_backup(self, job)
            next_run = job.get("_next_run_at")  # may be a datetime

            if last_dt is None:
                # Never backed up → run now
                if hasattr(self, "logger"):
                    self.logger.info(f"[{job.get('zip_name','job')}] No previous backup found → running now.")
                if hasattr(self, "create_backup"):
                    try:
                        # Use the queue system instead of direct execution
                        if hasattr(self, "_queue_backup"):
                            self._queue_backup(job)
                        else:
                            self.create_backup(job)
                        if hasattr(self, "logger"):
                            self.logger.success(f"[{job.get('zip_name','job')}] Backup queued successfully")
                    except Exception as e:
                        if hasattr(self, "logger"):
                            self.logger.error(f"[{job.get('zip_name','job')}] Backup failed: {e}")
                job["_next_run_at"] = now + timedelta(seconds=interval)
                changed = True
                continue

            due_at = last_dt + timedelta(seconds=interval)
            if isinstance(next_run, datetime):
                # respect any planned future run
                due_at = max(due_at, next_run)

            # Only run if significantly overdue (more than 5 minutes past due)
            overdue_threshold = due_at + timedelta(minutes=5)
            if now >= overdue_threshold:
                if hasattr(self, "logger"):
                    self.logger.info(f"[{job.get('zip_name','job')}] Significantly overdue since {due_at:%Y-%m-%d %H:%M:%S} → running now.")
                if hasattr(self, "create_backup"):
                    try:
                        # Use the queue system instead of direct execution
                        if hasattr(self, "_queue_backup"):
                            self._queue_backup(job)
                        else:
                            self.create_backup(job)
                        if hasattr(self, "logger"):
                            self.logger.success(f"[{job.get('zip_name','job')}] Overdue backup queued successfully")
                    except Exception as e:
                        if hasattr(self, "logger"):
                            self.logger.error(f"[{job.get('zip_name','job')}] Backup failed: {e}")
                job["_next_run_at"] = now + timedelta(seconds=interval)
                changed = True
            else:
                job["_next_run_at"] = due_at

        except Exception as e:
            if hasattr(self, "logger"):
                self.logger.error(f"ensure_overdue_jobs_run error for job {job.get('zip_name','?')}: {e}")

    # Ask UI to refresh if anything changed
    jf = getattr(self, "job_frame", None)
    if changed and jf and hasattr(jf, "update_job_list"):
        try:
            jf.update_job_list()
        except Exception:
            pass

def check_overdue_jobs_smart(self):
    """
    Smart overdue checking that respects job intervals.
    Only checks jobs when they're actually due or close to being due.
    """
    now = datetime.now()
    
    # Get the next check time for this manager instance
    next_check = getattr(self, "_next_smart_check", None)
    
    # If we haven't set a next check time yet, or it's time to check
    if next_check is None or now >= next_check:
        # Run the overdue check
        ensure_overdue_jobs_run(self)
        
        # Calculate the next check time based on the shortest job interval
        jobs = list(getattr(self, "active_jobs", []))
        if jobs:
            # Find the shortest interval among all jobs
            shortest_interval = min(_interval_seconds(self, job) for job in jobs)
            # Check again in half the shortest interval (to catch overdue jobs promptly)
            next_check_time = now + timedelta(seconds=shortest_interval // 2)
        else:
            # No jobs, check again in 5 minutes
            next_check_time = now + timedelta(minutes=5)
        
        # Store the next check time
        setattr(self, "_next_smart_check", next_check_time)
        
        # Only log the next check time once per session to reduce spam
        if hasattr(self, "logger") and not hasattr(self, "_logged_next_check"):
            self.logger.info(f"Next overdue check scheduled for: {next_check_time:%Y-%m-%d %H:%M:%S}")
            setattr(self, "_logged_next_check", True)

def run_scheduler_with_overdue(self):
    """
    Smart scheduler that:
    - checks for overdue jobs only on startup and 5 minutes after scheduled backups
    - NOTE: schedule.run_pending() is removed to prevent conflicts with main scheduler
    """
    # Only check for overdue jobs occasionally, not every second
    now = datetime.now()
    last_overdue_check = getattr(self, "_last_overdue_check", None)
    
    # Check for overdue jobs:
    # 1. On first run (last_overdue_check is None)
    # 2. Every 5 minutes after the last check
    if (last_overdue_check is None or 
        (now - last_overdue_check).total_seconds() >= 300):  # 5 minutes = 300 seconds
        
        ensure_overdue_jobs_run(self)
        self._last_overdue_check = now
        
        if hasattr(self, "logger"):
            self.logger.info(f"Overdue check completed. Next check in 5 minutes.")

def apply_overdue_patch(BackupManagerClass):
    """
    Monkey-patch your existing BackupManager (no invasive edits).
    Call this once at startup (e.g., in main.py) after importing BackupManager.
    """
    # Attach helpers
    setattr(BackupManagerClass, "get_latest_backup", get_latest_backup)
    setattr(BackupManagerClass, "_interval_seconds", _interval_seconds)
    setattr(BackupManagerClass, "ensure_overdue_jobs_run", ensure_overdue_jobs_run)
    setattr(BackupManagerClass, "check_overdue_jobs_smart", check_overdue_jobs_smart)

    # Wrap/replace run_scheduler safely
    # If you want to keep your original run_scheduler around:
    if not hasattr(BackupManagerClass, "_orig_run_scheduler"):
        setattr(BackupManagerClass, "_orig_run_scheduler",
                getattr(BackupManagerClass, "run_scheduler", lambda self: None))
    def _patched_run_scheduler(self):
        # First let the original do its work
        try:
            getattr(self, "_orig_run_scheduler")()
        except Exception:
            pass
        # Then our robust scheduler + overdue
        run_scheduler_with_overdue(self)
    setattr(BackupManagerClass, "run_scheduler", _patched_run_scheduler)
