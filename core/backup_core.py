import os
import json
import time
import zipfile
import schedule
import threading
import subprocess
import shutil
import glob
from datetime import datetime, timedelta
import tkinter as tk
from tkinter import ttk, StringVar, IntVar, SUNKEN, W, X, BOTTOM, END, Frame, filedialog, messagebox
from tkinter import *
from tkinter.scrolledtext import ScrolledText
from tkinter.filedialog import askdirectory
from tkinter.messagebox import showinfo, showerror, askyesno
from concurrent.futures import ThreadPoolExecutor
from queue import Queue
from core.logger import Logger

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
JOB_FILE = os.path.join(SCRIPT_DIR, "jobs.json")
LOG_FOLDER = os.path.join(SCRIPT_DIR, "logs")
CONFIG_FILE = os.path.join(SCRIPT_DIR, "config.json")


class BackupManager:
    def __init__(self):
        """Initialize backup manager"""
        print("Initializing BackupManager")  # Debug print
        self.active_jobs = []
        self.job_frame = None
        self.current_job = None
        self.monthly_backup_destination = None  # Store user's preferred monthly backup location

        # Single-worker executor to keep backups off UI thread and serialized
        self.executor = ThreadPoolExecutor(max_workers=1)
        
        # Backup queue for sequential execution
        self.backup_queue = Queue()
        self.backup_running = False
        self.backup_thread = None
        
        # Scheduler status tracking
        self._scheduler_running = False
        self._last_scheduler_run = None

        # Initialize logger
        try:
            self.logger = Logger()
            print("Logger initialized successfully")  # Debug print
            # self.logger.info("BackupManager initialized")
        except Exception as e:
            print(f"Failed to initialize logger: {str(e)}")
            raise

        # Load configuration
        self.load_config()

        # Now load jobs with better error handling
        try:
            self.load_jobs()
            # self.logger.info(f"Loaded {len(self.active_jobs)} jobs")
            
            # Log each job that was loaded
            # for i, job in enumerate(self.active_jobs):
            #     self.logger.info(f"Job {i}: {job.get('zip_name', 'unknown')} -> {job.get('destination', 'no destination')}")

            # Reschedule all jobs
            for job in self.active_jobs:
                try:
                    self.schedule_job(job)
                    # self.logger.info(f"Rescheduled job: {job['zip_name']}")
                except Exception as e:
                    self.logger.error(f"Failed to schedule job {job['zip_name']}: {str(e)}")

            # Schedule monthly archive
            self.schedule_monthly_archive()
            
            # Start the backup queue processor
            self._start_backup_queue_processor()

        except Exception as e:
            self.logger.error(f"Failed to load jobs: {str(e)}")
            self.active_jobs = []

    def set_job_frame(self, job_frame):
        """Set the job frame reference"""
        self.job_frame = job_frame
        # self.logger.info("Job frame reference set")

        # Initialize the job list properly
        if hasattr(self.job_frame, 'initialize_job_list'):
            self.job_frame.initialize_job_list()
        elif hasattr(self.job_frame, 'update_job_list'):
            self.job_frame.update_job_list()
        else:
            self.logger.warning("Job frame does not have initialize_job_list or update_job_list method")

    def run_scheduler(self):
        """Run any pending scheduled jobs and check for overdue jobs"""
        try:
            # Mark scheduler as running
            self._scheduler_running = True
            self._last_scheduler_run = datetime.now()
            
            # Use a single, efficient approach that handles both scheduled and overdue jobs
            self._run_smart_scheduler()
            
        except Exception as e:
            self.logger.error(f"Scheduler error: {str(e)}")
            self._scheduler_running = False
            if self.job_frame:
                self.job_frame.update_job_list()

    def _run_smart_scheduler(self):
        """Smart scheduler that handles each job individually on its own schedule"""
        try:
            import schedule
            
            # Debug logging - show what jobs are scheduled (only once)
            if not hasattr(self, '_scheduler_debug_logged'):
                jobs = schedule.get_jobs()
                self.logger.info(f"Scheduler has {len(jobs)} total jobs scheduled")
                for job in jobs:
                    next_run = getattr(job, 'next_run', None)
                    job_tag = getattr(job, 'tag', 'unknown')
                    if next_run:
                        self.logger.info(f"  - {job_tag}: next run at {next_run.strftime('%H:%M:%S')}")
                self._scheduler_debug_logged = True
            
            # Check each job individually to see if it's due
            now = datetime.now()
            jobs_executed = 0
            
            for job in schedule.get_jobs():
                if hasattr(job, 'next_run') and job.next_run and job.next_run <= now:
                    job_tag = getattr(job, 'tag', 'unknown')
                    self.logger.info(f"Job {job_tag} is due - executing now")
                    
                    # Execute this specific job
                    try:
                        job.run()
                        jobs_executed += 1
                        self.logger.info(f"Successfully executed job: {job_tag}")
                    except Exception as e:
                        self.logger.error(f"Failed to execute job {job_tag}: {str(e)}")
            
            # Log summary
            if jobs_executed > 0:
                self.logger.info(f"Scheduler executed {jobs_executed} individual job(s)")
                
        except Exception as e:
            self.logger.error(f"Error in smart scheduler: {str(e)}")

    def is_scheduler_running(self):
        """Check if the scheduler is running (has run within the last minute)"""
        if not hasattr(self, '_scheduler_running') or not hasattr(self, '_last_scheduler_run'):
            return False
        
        # Consider scheduler running if it has run within the last minute
        if self._scheduler_running and self._last_scheduler_run:
            time_since_last_run = (datetime.now() - self._last_scheduler_run).total_seconds()
            return time_since_last_run < 60  # 60 seconds = 1 minute
        
        return False

    def load_config(self):
        """Load configuration from file"""
        try:
            if os.path.exists(CONFIG_FILE):
                with open(CONFIG_FILE, 'r') as f:
                    config = json.load(f)
                    self.monthly_backup_destination = config.get('monthly_backup_destination')
            else:
                # Create default config
                self.save_config()
        except Exception as e:
            self.logger.error(f"Error loading config: {str(e)}")

    def save_config(self):
        """Save configuration to file"""
        try:
            config = {
                'monthly_backup_destination': self.monthly_backup_destination
            }
            with open(CONFIG_FILE, 'w') as f:
                json.dump(config, f, indent=4)
        except Exception as e:
            self.logger.error(f"Error saving config: {str(e)}")

    def set_monthly_backup_destination(self, destination):
        """Set the monthly backup destination"""
        self.monthly_backup_destination = destination
        self.save_config()

    def load_jobs(self):
        """Load jobs from file with improved error handling"""
        try:
            jobs_file = os.path.join(SCRIPT_DIR, 'backup_jobs.json')  # Use SCRIPT_DIR constant
            if os.path.exists(jobs_file):
                with open(jobs_file, 'r') as f:
                    data = f.read()
                    if not data.strip():
                        self.logger.warning("Jobs file is empty")
                        self.active_jobs = []
                        return

                    loaded_jobs = json.loads(data)
                    if not isinstance(loaded_jobs, list):
                        raise ValueError("Invalid jobs data format")

                    self.active_jobs = loaded_jobs
                    # self.logger.info(f"Successfully loaded {len(self.active_jobs)} jobs from {jobs_file}")
            else:
                # self.logger.info(f"No jobs file found at {jobs_file}. Starting fresh.")
                self.active_jobs = []

        except json.JSONDecodeError as e:
            self.logger.error(f"Error parsing jobs file: {str(e)}")
            self.active_jobs = []
        except Exception as e:
            self.logger.error(f"Error loading jobs: {str(e)}")
            self.active_jobs = []

    def add_job(self, job_details):
        """Add a new backup job"""
        try:
            # Check if job name already exists
            if any(job['zip_name'] == job_details['zip_name'] for job in self.active_jobs):
                raise ValueError(f"A job with name '{job_details['zip_name']}' already exists")

            # Add the job to active jobs
            self.active_jobs.append(job_details)

            # Save jobs to file
            self.save_jobs()

            # Schedule the job
            self.schedule_job(job_details)

            self.logger.success(f"Added new job: {job_details['zip_name']}")

        except Exception as e:
            self.logger.error(f"Error adding job: {str(e)}")
            raise

    def update_job(self, index, job_details):
        """Update an existing backup job"""
        try:
            # Check if new name conflicts with other jobs
            if job_details['zip_name'] != self.active_jobs[index]['zip_name']:
                if any(job['zip_name'] == job_details['zip_name'] for i, job in enumerate(self.active_jobs) if i != index):
                    raise ValueError(f"A job with name '{job_details['zip_name']}' already exists")

            # Clear existing schedule
            old_job_name = self.active_jobs[index]['zip_name']
            schedule.clear(old_job_name)

            # Update job
            self.active_jobs[index] = job_details

            # Save jobs to file
            self.save_jobs()

            # Schedule the updated job
            self.schedule_job(job_details)

            # Removed update job logging as requested

        except Exception as e:
            self.logger.error(f"Error updating job: {str(e)}")
            raise

    def save_jobs(self):
        """Save jobs to file"""
        try:
            jobs_file = os.path.join(SCRIPT_DIR, 'backup_jobs.json')  # Use SCRIPT_DIR constant
            with open(jobs_file, 'w') as f:
                json.dump(self.active_jobs, f, indent=4)
            # self.logger.info(f"Successfully saved {len(self.active_jobs)} jobs to {jobs_file}")
        except Exception as e:
            self.logger.error(f"Error saving jobs: {str(e)}")
            raise

    def schedule_job(self, job_details):
        """Schedule a backup job"""
        try:
            # Clear any existing jobs with the same name
            schedule.clear(job_details['zip_name'])

            # Calculate interval in seconds
            interval = self._calculate_interval(
                job_details['interval_value'],
                job_details['interval_unit']
            )

            # Create a copy of job details for the scheduler
            scheduler_job = job_details.copy()
            scheduler_job['is_scheduled'] = True

            # Submit actual backup to the queue for sequential execution
            def submit_job():
                self._queue_backup(scheduler_job)

            schedule.every(interval).seconds.do(submit_job).tag(job_details['zip_name'])
            self.logger.info(f"Scheduled job: {job_details['zip_name']} to run every {interval} seconds ({job_details['interval_value']} {job_details['interval_unit']})")

        except Exception as e:
            self.logger.error(f"Error scheduling job: {str(e)}")
            raise

    def _start_backup_queue_processor(self):
        """Start the backup queue processor thread"""
        if self.backup_thread is None or not self.backup_thread.is_alive():
            self.backup_thread = threading.Thread(target=self._process_backup_queue, daemon=True)
            self.backup_thread.start()
            self.logger.info("Backup queue processor started")

    def _process_backup_queue(self):
        """Process backups from the queue sequentially"""
        while True:
            try:
                # Wait for a backup job to be queued
                job_details = self.backup_queue.get()
                if job_details is None:  # Shutdown signal
                    break
                
                # Mark that a backup is running
                self.backup_running = True
                job_name = job_details.get('zip_name', '?')
                self.logger.info(f"Starting backup: {job_name}")
                
                try:
                    # Run the backup
                    self.create_backup(job_details)
                except Exception as e:
                    self.logger.error(f"Backup failed for {job_name}: {str(e)}")
                finally:
                    # Mark that backup is complete
                    self.backup_running = False
                    self.logger.info(f"Completed backup: {job_name}")
                    self.backup_queue.task_done()
                    
            except Exception as e:
                self.logger.error(f"Error in backup queue processor: {str(e)}")
                self.backup_running = False

    def _queue_backup(self, job_details):
        """Add a backup job to the queue for sequential execution"""
        job_name = job_details.get('zip_name', '?')
        
        # Check if this job is already queued to prevent duplicates
        if hasattr(self, '_recently_queued_jobs'):
            now = datetime.now()
            # Remove jobs older than 5 minutes from the recently queued list
            self._recently_queued_jobs = {
                name: time for name, time in self._recently_queued_jobs.items()
                if (now - time).total_seconds() < 300
            }
            
            # Check if this job was recently queued (within 2 minutes)
            if job_name in self._recently_queued_jobs:
                recent_time = self._recently_queued_jobs[job_name]
                if (now - recent_time).total_seconds() < 120:  # 2 minutes
                    self.logger.info(f"Skipping duplicate backup request for {job_name} (queued {int((now - recent_time).total_seconds())} seconds ago)")
                    return
        else:
            self._recently_queued_jobs = {}
        
        # Add to recently queued jobs
        self._recently_queued_jobs[job_name] = datetime.now()
        
        self.backup_queue.put(job_details)
        
        if self.backup_running:
            self.logger.info(f"Queued backup job: {job_name} (waiting for current backup to complete)")
        else:
            self.logger.info(f"Queued backup job: {job_name} (starting immediately)")
    
    def is_backup_running(self):
        """Check if a backup is currently running"""
        return self.backup_running
    
    def get_queue_size(self):
        """Get the number of backups waiting in the queue"""
        return self.backup_queue.qsize()
    
    def check_and_run_overdue_jobs(self):
        """Legacy method - now handled by smart scheduler. Kept for backward compatibility."""
        self.logger.info("check_and_run_overdue_jobs called - this is now handled by the smart scheduler")
        # The smart scheduler already handles this, so we don't need to do anything here

    def get_scheduled_jobs_status(self):
        """Get status of all scheduled jobs for debugging"""
        try:
            import schedule
            status = []
            for job in self.active_jobs:
                job_name = job.get('zip_name', 'unknown')
                scheduled_jobs = schedule.get_jobs(tag=job_name)
                
                if scheduled_jobs:
                    next_run = scheduled_jobs[0].next_run
                    status.append({
                        'job_name': job_name,
                        'scheduled': True,
                        'next_run': next_run.strftime("%Y-%m-%d %H:%M:%S") if next_run else "Not scheduled"
                    })
                else:
                    status.append({
                        'job_name': job_name,
                        'scheduled': False,
                        'next_run': "Not found in scheduler"
                    })
            return status
        except Exception as e:
            self.logger.error(f"Error getting scheduled jobs status: {str(e)}")
            return []

    def _run_backup_safe(self, job_details):
        """Run a backup safely in the worker thread."""
        try:
            # Queue the backup for sequential execution instead of running immediately
            self._queue_backup(job_details)
        except Exception as e:
            self.logger.error(f"Failed to queue backup for {job_details.get('zip_name', '?')}: {str(e)}")

    def _calculate_interval(self, value, unit):
        """Calculate interval in seconds based on value and unit"""
        try:
            value = int(value)
            if value <= 0:
                raise ValueError("Interval value must be positive")

            # Convert to seconds based on unit
            if unit == "minutes":
                return value * 60
            elif unit == "hours":
                return value * 3600
            elif unit == "days":
                return value * 86400
            else:
                raise ValueError(f"Invalid interval unit: {unit}")

        except ValueError as e:
            self.logger.error(f"Error calculating interval: {str(e)}")
            raise ValueError(f"Invalid interval: {str(e)}")

    def set_logger(self, logger):
        """Set the logger instance"""
        self.logger = logger

    def create_backup(self, job_details):
        """Create a backup according to job specifications"""
        try:
            job_name = job_details['zip_name']

            # Different start messages for scheduled vs manual backups
            if job_details.get('is_scheduled', False):
                self.logger.info(f"=== Starting Scheduled Backup for {job_name} ===")
            else:
                self.logger.info(f"=== Starting Manual Backup for {job_name} ===")

            # Choose backup method based on game type
            success = False
            if job_details['game_type'] == 'ARK':
                success = self.create_ark_backup(job_details)
            elif job_details['game_type'] == 'Palworld':
                success = self.create_palworld_backup(job_details)
            else:
                raise ValueError(f"Unknown game type: {job_details['game_type']}")

            if success:
                # Use success method instead of info for completion message
                self.logger.success(f"=== Backup Completed Successfully for {job_name} ===")
                return True

        except Exception as e:
            self.logger.error(f"Backup failed for {job_name}: {str(e)}")
            raise

    def create_palworld_backup(self, job_details):
        """Create a Palworld backup"""
        try:
            source_dir = self.normalize_path(job_details['source'])
            dest_dir = self.normalize_path(job_details['destination'])
            job_name = job_details['zip_name']

            # Find the SaveGames path and dynamic folder
            save_games_path = os.path.join(source_dir, "Pal", "Saved", "SaveGames", "0")
            try:
                dynamic_folders = [f for f in os.listdir(save_games_path) 
                                   if os.path.isdir(os.path.join(save_games_path, f))]
                if not dynamic_folders:
                    raise ValueError("No save folder found in SaveGames directory")
                dynamic_folder = dynamic_folders[0]
            except Exception as e:
                raise ValueError(f"Error finding Palworld save folder: {str(e)}")

            save_path = os.path.join(save_games_path, dynamic_folder)

            # Create backup paths
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            zip_filename = f"{job_name}_{timestamp}.zip"
            zip_path = os.path.join(dest_dir, zip_filename)
            tmp_path = zip_path + ".tmp"

            # Ensure destination directory exists
            os.makedirs(dest_dir, exist_ok=True)

            # Create the backup atomically
            with zipfile.ZipFile(tmp_path, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=5) as zipf:
                # Individual files to backup
                files_to_backup = ["Level.sav", "LevelMeta.sav"]

                # Add individual files
                for file in files_to_backup:
                    file_path = os.path.join(save_path, file)
                    if os.path.exists(file_path):
                        arcname = os.path.join("SaveGames", "0", dynamic_folder, file)
                        zipf.write(file_path, arcname)

                # Backup Players folder and contents
                players_path = os.path.join(save_path, "Players")
                if os.path.exists(players_path):
                    for root, _, files in os.walk(players_path):
                        root_rel = os.path.relpath(root, save_path)
                        for f in files:
                            file_path = os.path.join(root, f)
                            arcname = os.path.join("SaveGames", "0", dynamic_folder, root_rel, f)
                            zipf.write(file_path, arcname)

            os.replace(tmp_path, zip_path)

            # Verify backup
            if self._verify_backup(zip_path):
                self._cleanup_old_backups(dest_dir, job_details['keep_days'])
                return True
            else:
                raise ValueError("Backup verification failed")

        except Exception as e:
            raise

    def _should_backup_file(self, filename, job_details):
        """Check if file should be included in backup"""
        # Get file extension
        _, ext = os.path.splitext(filename)
        ext = ext.lower()

        # Check specific files
        if filename in job_details.get('specific_files', []):
            return True

        # Check extensions
        allowed_extensions = job_details.get('extensions', [])
        if allowed_extensions and ext not in allowed_extensions:
            return False

        # Check exclusion patterns
        exclude_patterns = job_details.get('exclude_patterns', [])
        for pattern in exclude_patterns:
            if pattern in filename.lower():
                return False

        return True

    def _verify_backup(self, zip_path):
        """Verify backup integrity"""
        try:
            with zipfile.ZipFile(zip_path, 'r') as zipf:
                # Test zip file integrity
                test_result = zipf.testzip()
                if test_result is not None:
                    self.logger.error(f"Corrupt file in backup: {test_result}")
                    return False
                return True
        except Exception as e:
            self.logger.error(f"Backup verification failed: {str(e)}")
            return False

    def _cleanup_old_backups(self, backup_dir, keep_days):
        """Remove backups older than specified days"""
        try:
            cutoff = datetime.now() - timedelta(days=keep_days)
            with os.scandir(backup_dir) as it:
                for entry in it:
                    if entry.is_file() and entry.name.endswith('.zip'):
                        if datetime.fromtimestamp(entry.stat().st_mtime) < cutoff:
                            os.remove(entry.path)
                            self.logger.info(f"Removed old backup: {entry.name}")
        except Exception as e:
            self.logger.warning(f"Error cleaning old backups: {str(e)}")

    def normalize_path(self, path):
        """Normalize path to use correct separators and handle potential issues"""
        if not path:
            raise ValueError("Path cannot be empty")
        return os.path.normpath(path)

    def validate_source_path(self, path, game_type):
        """Validate the source path structure based on game type"""
        if not path:
            raise ValueError("Source path cannot be empty")

        if not os.path.exists(path):
            raise ValueError(f"Source path does not exist: {path}")

        # Explicit game type check
        if game_type == 'ARK':
            # Check for ShooterGame directory
            shooter_path = os.path.join(path, "ShooterGame")
            if not os.path.exists(shooter_path):
                raise ValueError(f"ShooterGame directory not found in: {path}")
        elif game_type == 'Palworld':
            # Check for Pal/Saved/SaveGames path
            save_path = os.path.join(path, "Pal", "Saved", "SaveGames", "0")
            if not os.path.exists(save_path):
                raise ValueError(f"Invalid Palworld save path structure in: {path}")

            # Check if there's at least one save folder
            save_folders = [f for f in os.listdir(save_path) 
                          if os.path.isdir(os.path.join(save_path, f))]
            if not save_folders:
                raise ValueError("No save folder found in Palworld SaveGames directory")

        return self.normalize_path(path)

    def find_dynamic_folder(self, saved_arks_path):
        """Find the dynamic folder inside SavedArks without logging"""
        try:
            items = os.listdir(saved_arks_path)
            folders = [f for f in items if os.path.isdir(os.path.join(saved_arks_path, f))]

            if not folders:
                raise ValueError(f"No subdirectories found in SavedArks")

            return folders[0]

        except Exception as e:
            raise ValueError(f"Error finding dynamic folder: {str(e)}")

    def create_ark_backup(self, job_details):
        """Create an ARK backup"""
        try:
            source_dir = self.normalize_path(job_details['source'])
            dest_dir = self.normalize_path(job_details['destination'])
            selected_map = job_details.get('selected_map', '')
            
            # Validate source directory exists
            if not os.path.exists(source_dir):
                raise ValueError(f"Source directory does not exist: {source_dir}")
            
            if hasattr(self, "logger"):
                self.logger.info(f"Creating ARK backup from {source_dir} to {dest_dir}")

            # Create timestamp for backup
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            zip_filename = f"{job_details['zip_name']}_{timestamp}.zip"
            zip_path = os.path.join(dest_dir, zip_filename)
            tmp_path = zip_path + ".tmp"

            # Ensure destination directory exists
            os.makedirs(dest_dir, exist_ok=True)

            # Create the backup atomically
            with zipfile.ZipFile(tmp_path, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=5) as zipf:
                # 1. Player Saves and Map Save
                if job_details.get('include_save', False):
                    saved_arks_path = os.path.join(source_dir, "ShooterGame", "Saved", "SavedArks")
                    if os.path.exists(saved_arks_path):
                        map_folder = None
                        for folder in os.listdir(saved_arks_path):
                            folder_path = os.path.join(saved_arks_path, folder)
                            if os.path.isdir(folder_path):
                                files = os.listdir(folder_path)
                                if any(f.startswith(selected_map) and f.endswith('.ark') for f in files):
                                    map_folder = folder
                                    break

                        if map_folder:
                            map_dir_path = os.path.join(saved_arks_path, map_folder)
                            for file in os.listdir(map_dir_path):
                                if (file.endswith(('.arkprofile', '.arktribe')) or 
                                    (file.startswith(selected_map) and file.endswith('.ark'))):
                                    file_path = os.path.join(map_dir_path, file)
                                    arcname = os.path.join("SavedArks", map_folder, file)
                                    zipf.write(file_path, arcname)

                # 2. Server Files
                if job_details.get('include_server_config', False):
                    server_config_path = os.path.join(source_dir, "ShooterGame", "Saved", "Config", "WindowsServer")
                    if os.path.exists(server_config_path):
                        for file in ['GameUserSettings.ini', 'Game.ini']:
                            file_path = os.path.join(server_config_path, file)
                            if os.path.exists(file_path):
                                arcname = os.path.join("ServerConfig", file)
                                zipf.write(file_path, arcname)

                # 3. Plugin Config Files
                if job_details.get('include_config', False):
                    plugins_path = os.path.join(source_dir, "ShooterGame", "Binaries", "Win64", "ArkApi", "Plugins")
                    if os.path.exists(plugins_path):
                        for plugin_folder in os.listdir(plugins_path):
                            plugin_dir = os.path.join(plugins_path, plugin_folder)
                            if os.path.isdir(plugin_dir):
                                config_file = os.path.join(plugin_dir, "config.json")
                                if os.path.exists(config_file):
                                    arcname = os.path.join("Plugins", plugin_folder, "config.json")
                                    zipf.write(config_file, arcname)

            os.replace(tmp_path, zip_path)

            # Verify backup
            if os.path.exists(zip_path) and os.path.getsize(zip_path) > 0:
                self._cleanup_old_backups(dest_dir, job_details['keep_days'])
                return True
            else:
                raise ValueError("Backup file creation failed or file is empty")

        except Exception as e:
            raise

    def perform_monthly_backup(self, job, monthly_destination=None):
        """Perform monthly backup for a job"""
        try:
            # Get current date info
            today = datetime.now()
            month_name = today.strftime("%B")  # Full month name
            year_month = today.strftime("%Y-%m")  # YYYY-MM
            folder_name = f"{year_month}-{month_name}"

            # Define source and destination paths
            source_dir = job['destination']  # Where the regular backups are stored
            
            # Use provided destination, stored destination, or default
            if monthly_destination:
                dest_base = monthly_destination
            elif self.monthly_backup_destination:
                dest_base = self.monthly_backup_destination
            else:
                dest_base = r"C:\arkade\Arkade Shared Global\FOTM Backups"
            
            date_dir = os.path.join(dest_base, folder_name)

            # Determine game-specific subfolder
            if job['game_type'].upper() == 'ARK':
                game_folder = 'ASA'
            elif job['game_type'].upper() == 'PALWORLD':
                game_folder = 'PAL'
            else:
                raise ValueError(f"Unknown game type: {job['game_type']}")

            # Create full destination path with game-specific folder
            dest_dir = os.path.join(date_dir, game_folder)

            # Create destination directory structure if it doesn't exist
            os.makedirs(dest_dir, exist_ok=True)

            # Get list of ALL backup files from current month (not just job-specific ones)
            backup_files = []
            if os.path.exists(source_dir):
                for file in os.listdir(source_dir):
                    if file.endswith('.zip'):
                        file_path = os.path.join(source_dir, file)
                        file_date = datetime.fromtimestamp(os.path.getmtime(file_path))

                        # Check if file is from current month
                        if file_date.strftime("%Y%m") == today.strftime("%Y%m"):
                            backup_files.append((file_path, file_date))

            # Sort by date and get oldest two (first two after sorting by date)
            backup_files.sort(key=lambda x: x[1])
            files_to_copy = backup_files[:2]

            if not files_to_copy:
                raise ValueError(f"No backup files found for {job['zip_name']} in current month")

            # Copy the files
            copied_files = []
            for file_path, _ in files_to_copy:
                file_name = os.path.basename(file_path)
                dest_path = os.path.join(dest_dir, file_name)
                shutil.copy2(file_path, dest_path)
                copied_files.append(f"{game_folder}/{file_name}")
                self.logger.info(f"Copied {file_name} to monthly archive/{game_folder}")

            return copied_files

        except Exception as e:
            self.logger.error(f"Error creating monthly backup: {str(e)}")
            raise

    def archive_monthly_backups(self, monthly_destination=None):
        """Preview and execute monthly backups for all jobs"""
        try:
            if not self.active_jobs:
                messagebox.showwarning("No Jobs", "No backup jobs are configured.")
                return

            # Build preview of what will be archived
            preview_text = "The following files will be archived:\n\n"

            # Check each job's available files first
            job_previews = {}
            for job in self.active_jobs:
                source_dir = job['destination']
                game_type = job['game_type'].upper()
                game_folder = 'ASA' if game_type == 'ARK' else 'PAL'

                backup_files = []
                if os.path.exists(source_dir):
                    for file in os.listdir(source_dir):
                        if file.endswith('.zip'):
                            file_path = os.path.join(source_dir, file)
                            file_date = datetime.fromtimestamp(os.path.getmtime(file_path))

                            # Check if file is from current month
                            if file_date.strftime("%Y%m") == datetime.now().strftime("%Y%m"):
                                backup_files.append((file_path, file_date))

                # Sort by date and get oldest two (first two after sorting by date)
                backup_files.sort(key=lambda x: x[1])
                files_to_copy = backup_files[:2]

                # Add to preview
                preview_text += f"Job: {job['zip_name']} ({game_folder})\n"
                if files_to_copy:
                    preview_text += f"- Will archive oldest {len(files_to_copy)} files:\n"
                    for file_path, date in files_to_copy:
                        preview_text += f"  • {os.path.basename(file_path)} ({date.strftime('%Y-%m-%d %H:%M')})\n"
                else:
                    preview_text += "- No files available for current month\n"
                preview_text += "\n"

                job_previews[job['zip_name']] = files_to_copy

            # Show preview and ask for confirmation
            result = messagebox.askyesno("Confirm Monthly Archive", 
                f"{preview_text}\nDo you want to proceed with archiving these files?")

            if result:
                self.logger.info("=== Starting Monthly Archive Process ===")
                success_count = 0

                for job in self.active_jobs:
                    try:
                        copied_files = self.perform_monthly_backup(job, monthly_destination)
                        if copied_files:
                            success_count += 1
                    except Exception as e:
                        messagebox.showerror("Archive Error", 
                            f"Error archiving {job['zip_name']}: {str(e)}")

                if success_count > 0:
                    self.logger.info(f"=== Monthly Archive Complete: {success_count} job(s) archived ===")
                    messagebox.showinfo("Archive Complete", 
                        f"Successfully archived files for {success_count} job(s)")

        except Exception as e:
            messagebox.showerror("Error", f"Failed to execute monthly archive: {str(e)}")

    def schedule_monthly_archive(self):
        """Schedule the monthly archive to run on the first day of each month at 11 PM"""
        try:
            # Clear any existing monthly archive schedule
            schedule.clear('monthly_archive')

            # Schedule the monthly archive
            schedule.every().day.at("23:00").do(
                self._check_and_run_monthly_archive
            ).tag('monthly_archive')

            self.logger.info("Monthly archive scheduled for first day of each month at 11 PM")

        except Exception as e:
            self.logger.error(f"Error scheduling monthly archive: {str(e)}")
            raise

    def _check_and_run_monthly_archive(self):
        """Check if it's the first day of the month and run the archive if it is"""
        if datetime.now().day == 1:
            self.archive_monthly_backups()

    def get_monthly_backup_status(self):
        """Get the status of monthly backups for all jobs"""
        try:
            status_data = []
            current_month = datetime.now().strftime("%Y%m")
            
            for job in self.active_jobs:
                source_dir = job['destination']
                game_type = job['game_type'].upper()
                game_folder = 'ASA' if game_type == 'ARK' else 'PAL'
                
                job_status = {
                    'job_name': job['zip_name'],
                    'game_type': game_folder,
                    'available_files': [],
                    'archived_files': [],
                    'can_archive': False
                }
                
                # Get available backup files for this month (all .zip files, not just job-specific ones)
                if os.path.exists(source_dir):
                    backup_files = []
                    for file in os.listdir(source_dir):
                        if file.endswith('.zip'):
                            file_path = os.path.join(source_dir, file)
                            file_date = datetime.fromtimestamp(os.path.getmtime(file_path))
                            
                            if file_date.strftime("%Y%m") == current_month:
                                backup_files.append((file_path, file_date))
                    
                    # Sort by date and get first two
                    backup_files.sort(key=lambda x: x[1])
                    job_status['available_files'] = backup_files[:2]
                    job_status['can_archive'] = len(backup_files) >= 1
                
                # Check if already archived (look in default monthly destination)
                default_monthly_dir = os.path.join(
                    r"C:\arkade\Arkade Shared Global\FOTM Backups",
                    f"{datetime.now().strftime('%Y-%m')}-{datetime.now().strftime('%B')}",
                    game_folder
                )
                
                if os.path.exists(default_monthly_dir):
                    archived_files = []
                    for file in os.listdir(default_monthly_dir):
                        if file.endswith('.zip'):
                            archived_files.append(file)
                    job_status['archived_files'] = archived_files
                
                status_data.append(job_status)
            
            return status_data
            
        except Exception as e:
            self.logger.error(f"Error getting monthly backup status: {str(e)}")
            return []

    def show_monthly_archive_status(self):
        """Show the status of monthly archives"""
        try:
            status_data = self.get_monthly_backup_status()
            
            if not status_data:
                messagebox.showinfo("Monthly Archive Status", "No backup jobs configured.")
                return
            
            status_text = "Monthly Archive Status:\n\n"
            
            for job_status in status_data:
                status_text += f"Job: {job_status['job_name']} ({job_status['game_type']})\n"
                
                if job_status['archived_files']:
                    status_text += f"✓ Already archived: {len(job_status['archived_files'])}/2 files\n"
                    for file in job_status['archived_files']:
                        status_text += f"  - {file}\n"
                elif job_status['can_archive']:
                    status_text += f"○ Available to archive (oldest {len(job_status['available_files'])} files):\n"
                    for file_path, date in job_status['available_files']:
                        status_text += f"  - {os.path.basename(file_path)} ({date.strftime('%Y-%m-%d %H:%M')})\n"
                else:
                    status_text += "✗ No backups available for current month\n"
                
                status_text += "\n"
            
            messagebox.showinfo("Monthly Archive Status", status_text)
            
        except Exception as e:
            self.logger.error(f"Error showing archive status: {str(e)}")
            messagebox.showerror("Error", f"Could not get archive status: {str(e)}")

    def delete_job(self, index):
        """Delete a backup job by index"""
        try:
            if 0 <= index < len(self.active_jobs):
                # Get job name for logging
                job_name = self.active_jobs[index]['zip_name']

                # Clear any scheduled tasks for this job
                schedule.clear(job_name)

                # Remove the job from active jobs
                del self.active_jobs[index]

                # Save updated jobs list
                self.save_jobs()

                self.logger.success(f"Successfully deleted job: {job_name}")
            else:
                raise ValueError("Invalid job index")

        except Exception as e:
            self.logger.error(f"Error deleting job: {str(e)}")
            raise

    def shutdown(self):
        """Shutdown the backup manager and stop the queue processor"""
        try:
            # Send shutdown signal to queue processor
            if self.backup_queue:
                self.backup_queue.put(None)
            
            # Wait for queue processor to finish
            if self.backup_thread and self.backup_thread.is_alive():
                self.backup_thread.join(timeout=5.0)
            
            # Shutdown executor
            if self.executor:
                self.executor.shutdown(wait=True)
                
            self.logger.info("Backup manager shutdown complete")
        except Exception as e:
            self.logger.error(f"Error during backup manager shutdown: {str(e)}")
