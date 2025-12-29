import tkinter as tk
import glob
import os
from datetime import datetime, timedelta
from tkinter import ttk, StringVar, IntVar, SUNKEN, W, X, BOTTOM, END, Frame, filedialog, messagebox
from tkinter.messagebox import askyesno, showinfo, showerror
from tkinter.scrolledtext import ScrolledText
from tabs.joblist_helpers import render_last_save_and_size

from core.theme import ModernTheme, CheckboxWithSymbol
from core.logger import Logger
from core.backup_core import BackupManager


class ToolTip:
    """Create a tooltip for a given widget"""
    def __init__(self, widget, text):
        self.widget = widget
        self.text = text
        self.tooltip = None
        self.widget.bind('<Enter>', self.show_tooltip)
        self.widget.bind('<Leave>', self.hide_tooltip)

    def show_tooltip(self, event=None):
        """Display the tooltip"""
        x, y, _, _ = self.widget.bbox("insert")
        x += self.widget.winfo_rootx() + 25
        y += self.widget.winfo_rooty() + 25

        # Create tooltip window
        self.tooltip = tk.Toplevel(self.widget)
        self.tooltip.wm_overrideredirect(True)
        self.tooltip.wm_geometry(f"+{x}+{y}")

        # Create tooltip label
        label = tk.Label(
            self.tooltip,
            text=self.text,
            justify='left',
            relief='solid',
            borderwidth=1,
            background='#ffffe0',
            foreground='black',
            padx=5,
            pady=5
        )
        label.pack()

    def hide_tooltip(self, event=None):
        """Hide the tooltip"""
        if self.tooltip:
            self.tooltip.destroy()
            self.tooltip = None



class JobFrame(ttk.Frame):
    """Enhanced job management interface"""
    def __init__(self, parent, theme_var, backup_manager, logger):
        ttk.Frame.__init__(self, parent)
        self.theme_var = theme_var
        self.backup_manager = backup_manager
        self.logger = logger
        self.selected_job_index = None

        self.create_variables()
        self.create_widgets()
        # Don't call _try_load_jobs or update_job_list here - let the backup manager handle it
        # when set_job_frame is called


    def create_variables(self):
        """Initialize all variables used in the job frame"""
        # Existing ARK variables
        self.source_var = StringVar()
        self.dest_var = StringVar()
        self.zip_name_var = StringVar()
        self.interval_value_var = StringVar()
        self.interval_unit_var = StringVar(value="minutes")
        self.keep_days_var = StringVar(value="20")
        self.game_type_var = StringVar(value="ARK")  # New variable for game type

        # ARK-specific variables
        self.selected_map_var = StringVar()
        self.include_config_files = IntVar()
        self.include_save_files = IntVar()
        self.include_server_config = IntVar()

        # Map options for ARK
        self.map_options = [
            "TheCenter_WP.ark",
            "Aberration_WP.ark",
            "Extinction_WP.ark",
            "TheIsland_WP.ark",
            "ScorchedEarth_WP.ark",
            "Svartalfheim_WP.ark",
            "Astraeos_WP.ark",
            "Forglar_WP.ark",
            "Amissa_WP.ark",
            "Ragnarok_WP.ark",
            "Valguero_WP.ark",
            "LostColony_WP.ark"
        ]

    def create_widgets(self):
        """Create all widgets for the job frame"""
        # Create main canvas with scrollbar
        self.canvas = tk.Canvas(self)
        scrollbar = ttk.Scrollbar(self, orient="vertical", command=self.canvas.yview)
        self.scrollable_frame = ttk.Frame(self.canvas)

        # Configure canvas
        self.scrollable_frame.bind(
            "<Configure>",
            lambda e: self.canvas.configure(scrollregion=self.canvas.bbox("all"))
        )
        self.canvas.create_window((0, 0), window=self.scrollable_frame, anchor="nw")
        self.canvas.configure(yscrollcommand=scrollbar.set)

        # Pack scrollbar and canvas
        scrollbar.pack(side="right", fill="y")
        self.canvas.pack(side="left", fill="both", expand=True)

        # Add mousewheel scrolling
        self.canvas.bind_all("<MouseWheel>", self._on_mousewheel)

        # Create form section
        form_frame = ttk.LabelFrame(self.scrollable_frame, text="Job Details")
        form_frame.pack(fill='x', padx=10, pady=5)

        # Create form fields
        self.create_form_fields(form_frame)

        # Create buttons - Moved above job list
        self.create_buttons()

        # Create job list
        self.create_job_list()

    def _on_mousewheel(self, event):
        """Handle mousewheel scrolling"""
        self.canvas.yview_scroll(int(-1*(event.delta/120)), "units")

    def create_form_fields(self, parent):
        """Create form input fields"""
        # Game Type Selection
        game_type_frame = ttk.Frame(parent)
        game_type_frame.pack(fill='x', padx=5, pady=5)
        ttk.Label(game_type_frame, text="Game Type:").pack(side='left')
        game_type_combo = ttk.Combobox(
            game_type_frame,
            textvariable=self.game_type_var,
            values=["ARK", "Palworld"],
            state="readonly"
        )
        game_type_combo.pack(side='left', padx=5)
        game_type_combo.bind('<<ComboboxSelected>>', self.on_game_type_changed)

        # Source Directory
        source_frame = ttk.Frame(parent)
        source_frame.pack(fill='x', padx=5, pady=5)
        ttk.Label(source_frame, text="Source Directory:").pack(side='left')
        ttk.Entry(source_frame, textvariable=self.source_var).pack(side='left', fill='x', expand=True, padx=5)
        tk.Button(source_frame, text="Browse", command=lambda: self.browse_directory(self.source_var),
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side='right')

        # Destination Directory
        dest_frame = ttk.Frame(parent)
        dest_frame.pack(fill='x', padx=5, pady=5)
        ttk.Label(dest_frame, text="Destination Directory:").pack(side='left')
        ttk.Entry(dest_frame, textvariable=self.dest_var).pack(side='left', fill='x', expand=True, padx=5)
        tk.Button(dest_frame, text="Browse", command=lambda: self.browse_directory(self.dest_var),
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side='right')

        # Create container for game-specific options
        self.game_specific_frame = ttk.Frame(parent)
        self.game_specific_frame.pack(fill='x', padx=5, pady=5)

        # Initial game type setup
        self.on_game_type_changed(None)

    def browse_directory(self, var):
        """Browse for a directory and validate based on game type"""
        directory = filedialog.askdirectory()
        if directory:
            try:
                # Only validate if this is the source directory
                if var == self.source_var:
                    game_type = self.game_type_var.get()
                    validated_path = self.backup_manager.validate_source_path(directory, game_type)
                    var.set(validated_path)
                else:
                    # For destination directory, just normalize the path
                    var.set(self.backup_manager.normalize_path(directory))
            except ValueError as e:
                messagebox.showerror("Invalid Directory", str(e))
                var.set("")  # Clear the invalid path

    def create_job_list(self):
        """Create the job list section with columns"""
        list_frame = ttk.LabelFrame(self.scrollable_frame, text="Active Jobs")
        list_frame.pack(fill='x', padx=10, pady=5)

        # Create Treeview with columns
        self.job_tree = ttk.Treeview(
            list_frame, 
            columns=("name", "interval", "NextSave", "LastSave", "LastSize"), 
            show="headings", 
            height=10
        )

        # Define columns
        self.job_tree.heading("name", text="Job Name")
        self.job_tree.heading("interval", text="Interval")
        self.job_tree.heading("NextSave", text="Next Save")
        self.job_tree.heading("LastSave", text="Last Save")
        self.job_tree.heading("LastSize", text="File Size")

        # Set column widths
        self.job_tree.column("name", width=150)
        self.job_tree.column("interval", width=100)
        self.job_tree.column("NextSave", width=150)
        self.job_tree.column("LastSave", width=150)
        self.job_tree.column("LastSize", width=100)

        # Add scrollbar
        scrollbar = ttk.Scrollbar(list_frame, orient="vertical", command=self.job_tree.yview)
        self.job_tree.configure(yscrollcommand=scrollbar.set)

        # Bind selection event
        self.job_tree.bind('<<TreeviewSelect>>', self.on_job_selected)

        # Pack widgets
        self.job_tree.pack(side='left', fill='both', expand=True)
        scrollbar.pack(side='right', fill='y')
        
        # Debug: Check if tree is properly configured
        # print(f"DEBUG: Treeview created with columns: {self.job_tree['columns']}")
        # print(f"DEBUG: Treeview show setting: {self.job_tree['show']}")
        # print(f"DEBUG: Treeview height: {self.job_tree['height']}")

    def get_next_save_time(self, job):
        """Calculate the next save time based on job schedule or stored next_run."""
        # Preferred: schedule.get_jobs(tag=zip_name) if your manager tags jobs by zip_name
        try:
            import schedule
            jobs = schedule.get_jobs(job.get('zip_name'))
            if jobs:
                nxt = jobs[0].next_run
                if nxt:
                    return nxt.strftime("%Y-%m-%d %H:%M")
        except Exception:
            pass

    # Fallback: if BackupManager stored a computed next run time
        nxt = job.get("_next_run_at")
        try:
            if nxt:
                # nxt may be a datetime or iso string
                from datetime import datetime
                if isinstance(nxt, str):
                    try:
                        # common iso format
                        nxt = datetime.fromisoformat(nxt)
                    except Exception:
                        nxt = None
                if nxt:
                    return nxt.strftime("%Y-%m-%d %H:%M")
        except Exception:
            pass

        return "Not Scheduled"
    
    def _try_load_jobs(self):
        """
        Try to load jobs from disk via your manager if it exposes a loader.
        Safe no-op if not present.
        """
        for fn in ("load_jobs", "read_jobs", "load_jobs_from_disk", "load_from_file"):
            if hasattr(self.backup_manager, fn):
                try:
                    getattr(self.backup_manager, fn)()
                    if hasattr(self.logger, "info"):
                        self.logger.info(f"Jobs loaded via {fn}.")
                    break
                except Exception as e:
                    if hasattr(self.logger, "warning"):
                        self.logger.warning(f"Job load via {fn} failed: {e}")
        


    def _get_jobs(self):
        """
        Return a non-empty list of job dicts from the UI or manager.
        Checks multiple common attributes and falls back to a one-time load.
        """
        # Debug logging removed as requested
        
        # First, try to get jobs directly from the backup manager
        if hasattr(self.backup_manager, 'active_jobs'):
            jobs = self.backup_manager.active_jobs
            # print(f"DEBUG: Found active_jobs: {len(jobs) if isinstance(jobs, list) else 'not a list'}")  # Console fallback
                    # if hasattr(self.logger, "info"):
        #     self.logger.info(f"Found active_jobs: {len(jobs) if isinstance(jobs, list) else 'not a list'}")
            if isinstance(jobs, list) and jobs:
                return jobs

        # Try to load jobs if they're not already loaded
        # print("DEBUG: Trying to load jobs...")  # Console fallback
        # if hasattr(self.logger, "info"):
        #     self.logger.info("Trying to load jobs...")
        self._try_load_jobs()
        
        # Check again after loading
        if hasattr(self.backup_manager, 'active_jobs'):
            jobs = self.backup_manager.active_jobs
            # print(f"DEBUG: After loading, active_jobs: {len(jobs) if isinstance(jobs, list) else 'not a list'}")  # Console fallback
            # if hasattr(self.logger, "info"):
            #     self.logger.info(f"After loading, active_jobs: {len(jobs) if isinstance(jobs, list) else 'not a list'}")
            if isinstance(jobs, list) and jobs:
                return jobs

        # Fallback: check other possible attributes
        for name in ("jobs", "job_list", "job_data"):
            if hasattr(self.backup_manager, name):
                lst = getattr(self.backup_manager, name)
                if isinstance(lst, dict):
                    lst = list(lst.values())
                if isinstance(lst, list) and lst:
                                    # print(f"DEBUG: Found jobs in {name}: {len(lst)}")  # Console fallback
                # if hasattr(self.logger, "info"):
                #     self.logger.info(f"Found jobs in {name}: {len(lst)}")
                    return lst

                    # print("DEBUG: No jobs found")  # Console fallback
        # if hasattr(self.logger, "info"):
        #     self.logger.info("No jobs found")
        return []



    def update_job_list(self):
        """
        Rebuild Treeview rows, preserve selection & scroll, and fill computed columns.
        This reads jobs from UI or manager (whichever actually has them).
        """
        tree = self.job_tree
        
        # Check if tree is properly initialized
        if not hasattr(self, 'job_tree') or self.job_tree is None:
            if hasattr(self.logger, "error"):
                self.logger.error("Job tree is not initialized")
            return

        # Debug logging removed as requested

        # remember selection (by job display name) and scroll
        selected_name = None
        sel = tree.selection()
        if sel:
            try:
                vals = tree.item(sel[0], "values")
                if isinstance(vals, (list, tuple)) and vals:
                    selected_name = vals[0]
            except Exception:
                pass
        try:
            y0, _ = tree.yview()
        except Exception:
            y0 = 0.0

        jobs = list(self._get_jobs())
        cols = list(getattr(tree, "columns", ()))
        
        # clear rows
        for iid in tree.get_children():
            tree.delete(iid)

        # Debug logging removed as requested
        
        # Check if columns are properly set
        if not cols or len(cols) == 0:
            # if hasattr(self.logger, "warning"):
            #     self.logger.warning("Tree columns are empty, reconfiguring...")
            # Reconfigure the tree columns if they're lost
            tree.configure(columns=("name", "interval", "NextSave", "LastSave", "LastSize"))
            # Also reconfigure the headings and column widths
            tree.heading("name", text="Job Name")
            tree.heading("interval", text="Interval")
            tree.heading("NextSave", text="Next Save")
            tree.heading("LastSave", text="Last Save")
            tree.heading("LastSize", text="File Size")
            tree.column("name", width=150)
            tree.column("interval", width=100)
            tree.column("NextSave", width=150)
            tree.column("LastSave", width=150)
            tree.column("LastSize", width=100)
            cols = ("name", "interval", "NextSave", "LastSave", "LastSize")
        else:
            if hasattr(self.logger, "info"):
                self.logger.info(f"Tree columns found: {cols}")

        match_item = None
        for job in jobs:
            name = job.get("zip_name") or job.get("name") or "job"

                    # Debug logging removed as requested

            # placeholders sized to columns (helper will fill)
            values = []
            for col in cols:
                cl = str(col).lower()
                if cl in ("name", "job", "zip_name", "zipname"):
                    values.append(name)
                else:
                    values.append("")

            item_id = tree.insert("", "end", text=name, values=tuple(values))
            
            # Manually set some values to test if the tree is working
            try:
                tree.set(item_id, "name", name)
                tree.set(item_id, "interval", f"{job.get('interval_value', '30')} {job.get('interval_unit', 'min')}")
                tree.set(item_id, "NextSave", "Not Scheduled")
                tree.set(item_id, "LastSave", "Never")
                tree.set(item_id, "LastSize", "N/A")
            except Exception as e:
                if hasattr(self.logger, "error"):
                    self.logger.error(f"Error setting values for job {name}: {e}")

            # fill LastSave/LastSize/NextSave/Interval for THIS row
            try:
                render_last_save_and_size(tree, item_id, job, self.backup_manager)
            except Exception as e:
                if hasattr(self.logger, "error"):
                    self.logger.error(f"Error rendering job {name}: {e}")
                # Fallback to manual values if render fails
                try:
                    tree.set(item_id, "NextSave", "Not Scheduled")
                    tree.set(item_id, "LastSave", "Never")
                    tree.set(item_id, "LastSize", "N/A")
                except Exception as fallback_error:
                    if hasattr(self.logger, "error"):
                        self.logger.error(f"Error setting fallback values for job {name}: {fallback_error}")

            if selected_name and name == selected_name:
                match_item = item_id

        # restore selection & scroll
        if match_item:
            try:
                tree.selection_set(match_item)
                tree.focus(match_item)
            except Exception:
                pass
        try:
            tree.yview_moveto(y0)
        except Exception:
            pass

        # friendly log if empty
        if not jobs and hasattr(self.logger, "info"):
            self.logger.info("No jobs found. Add a job or load from disk.")
        
        # Force tree to update and redraw
        tree.update_idletasks()




    def get_last_save_time(self, job):
        """Get the timestamp of the last backup for a job"""
        try:
            dest_dir = job['destination']
            prefix = job['zip_name']
            latest_mtime = None
            with os.scandir(dest_dir) as it:
                for entry in it:
                    if entry.is_file() and entry.name.startswith(prefix) and entry.name.endswith('.zip'):
                        m = entry.stat().st_mtime
                        if latest_mtime is None or m > latest_mtime:
                            latest_mtime = m
            if latest_mtime is None:
                return "Never"
            return datetime.fromtimestamp(latest_mtime).strftime("%Y-%m-%d %H:%M")
        except Exception:
            return "Never"

    def get_last_save_size(self, job):
        """Get the size of the last backup file"""
        try:
            dest_dir = job['destination']
            prefix = job['zip_name']
            latest = None
            latest_size = None
            with os.scandir(dest_dir) as it:
                for entry in it:
                    if entry.is_file() and entry.name.startswith(prefix) and entry.name.endswith('.zip'):
                        m = entry.stat().st_mtime
                        if latest is None or m > latest:
                            latest = m
                            latest_size = entry.stat().st_size
            if latest is None or latest_size is None:
                return "N/A"
            size_bytes = latest_size
            if size_bytes < 1024:
                return f"{size_bytes} B"
            elif size_bytes < 1024 * 1024:
                return f"{size_bytes/1024:.1f} KB"
            elif size_bytes < 1024 * 1024 * 1024:
                return f"{size_bytes/(1024*1024):.1f} MB"
            else:
                return f"{size_bytes/(1024*1024*1024):.1f} GB"
        except Exception:
            return "N/A"

    def on_job_selected(self, event):
        """Handle job selection"""
        selection = self.job_tree.selection()
        if not selection:
            self.selected_job_index = None
            return

        # Get the selected item
        item = self.job_tree.item(selection[0])
        job_name = item['values'][0]  # First column contains job name

        # Find the job index and load its details
        for i, job in enumerate(self.backup_manager.active_jobs):
            if job['zip_name'] == job_name:
                self.selected_job_index = i
                # Set game type first so UI updates properly
                self.game_type_var.set(job['game_type'])
                # Trigger UI update for game type
                self.on_game_type_changed(None)
                # Then load the job details
                self.load_job_details(job)
                break

    def create_buttons(self):
        """Create action buttons"""
        button_frame = ttk.Frame(self.scrollable_frame)
        button_frame.pack(fill='x', padx=10, pady=5)

        # Add Job button
        tk.Button(
            button_frame,
            text="Add Job",
            command=self.save_job,
            relief="raised",
            borderwidth=3,
            bg=self.get_button_color(),
            fg=self.get_button_text_color(),
            activebackground=self.get_button_hover_color(),
            activeforeground=self.get_button_text_color()
        ).pack(side='left', padx=5)

        # Update Job button
        tk.Button(
            button_frame,
            text="Update Job",
            command=self.update_selected_job,
            relief="raised",
            borderwidth=3,
            bg=self.get_button_color(),
            fg=self.get_button_text_color(),
            activebackground=self.get_button_hover_color(),
            activeforeground=self.get_button_text_color()
        ).pack(side='left', padx=5)

        # Delete Job button
        tk.Button(
            button_frame,
            text="Delete Job",
            command=self.delete_job,
            relief="raised",
            borderwidth=3,
            bg=self.get_button_color(),
            fg=self.get_button_text_color(),
            activebackground=self.get_button_hover_color(),
            activeforeground=self.get_button_text_color()
        ).pack(side='left', padx=5)

        # Run Now button
        tk.Button(
            button_frame,
            text="Run Now",
            command=self.run_selected_job,
            relief="raised",
            borderwidth=3,
            bg=self.get_button_color(),
            fg=self.get_button_text_color(),
            activebackground=self.get_button_hover_color(),
            activeforeground=self.get_button_text_color()
        ).pack(side='left', padx=5)

        # Clear Form button
        tk.Button(
            button_frame,
            text="Clear Form",
            command=self.clear_form,
            relief="raised",
            borderwidth=3,
            bg=self.get_button_color(),
            fg=self.get_button_text_color(),
            activebackground=self.get_button_hover_color(),
            activeforeground=self.get_button_text_color()
        ).pack(side='left', padx=5)

        # Refresh Jobs button
        tk.Button(
            button_frame,
            text="Refresh Jobs",
            command=self.refresh_jobs,
            relief="raised",
            borderwidth=3,
            bg=self.get_button_color(),
            fg=self.get_button_text_color(),
            activebackground=self.get_button_hover_color(),
            activeforeground=self.get_button_text_color()
        ).pack(side='left', padx=5)

        # Backup Status Frame
        status_frame = ttk.Frame(button_frame)
        status_frame.pack(side='right', padx=5)
        
        # Status Label
        self.status_label = ttk.Label(status_frame, text="Status: Ready")
        self.status_label.pack(side='left')
        
        # Update status periodically
        self.after(2000, self._update_status)

        # Monthly Backup buttons
        ttk.Separator(button_frame, orient='vertical').pack(side='left', fill='y', padx=10)
        
        tk.Button(
            button_frame,
            text="Monthly Status",
            command=self.show_monthly_status,
            relief="raised",
            borderwidth=3,
            bg=self.get_button_color(),
            fg=self.get_button_text_color(),
            activebackground=self.get_button_hover_color(),
            activeforeground=self.get_button_text_color()
        ).pack(side='left', padx=5)
        
        tk.Button(
            button_frame,
            text="Run Monthly Backup",
            command=self.run_monthly_backup,
            relief="raised",
            borderwidth=3,
            bg=self.get_button_color(),
            fg=self.get_button_text_color(),
            activebackground=self.get_button_hover_color(),
            activeforeground=self.get_button_text_color()
        ).pack(side='left', padx=5)





    def _update_status(self):
        """Update the backup status display"""
        try:
            # Get backup status
            is_backup_running = False
            queue_size = 0
            if hasattr(self.backup_manager, 'is_backup_running') and hasattr(self.backup_manager, 'get_queue_size'):
                is_backup_running = self.backup_manager.is_backup_running()
                queue_size = self.backup_manager.get_queue_size()
            
            # Get scheduler status
            is_scheduler_running = False
            if hasattr(self.backup_manager, 'is_scheduler_running'):
                is_scheduler_running = self.backup_manager.is_scheduler_running()
            
            # Build status text
            if is_backup_running:
                status_text = "Status: Backup Running"
                if queue_size > 0:
                    status_text += f" ({queue_size} queued)"
            elif queue_size > 0:
                status_text = f"Status: {queue_size} Backup(s) Queued"
            else:
                status_text = "Status: Ready"
            
            # Add scheduler status
            if is_scheduler_running:
                status_text += " | Scheduler: Running"
            else:
                status_text += " | Scheduler: Not Running"
            
            if hasattr(self, 'status_label'):
                self.status_label.config(text=status_text)
                
                # Set color based on scheduler status using theme colors
                colors = ModernTheme.get_colors(self.theme_var.get())
                if is_scheduler_running:
                    self.status_label.config(foreground=colors['success'])
                else:
                    self.status_label.config(foreground=colors['error'])
                    
        except Exception as e:
            if hasattr(self.logger, "error"):
                self.logger.error(f"Error updating status: {str(e)}")
        finally:
            # Schedule next update
            self.after(2000, self._update_status)

    def save_job(self):
        """Save the current job"""
        try:
            job_details = self.get_job_details()
            if self.selected_job_index is not None:
                # Update existing job (schedules inside update_job)
                self.backup_manager.update_job(self.selected_job_index, job_details)
            else:
                # Add new job (schedules inside add_job)
                self.backup_manager.add_job(job_details)
            self.update_job_list()
            self.clear_form()
        except Exception as e:
            messagebox.showerror("Error", str(e))

    def update_selected_job(self):
        """Update the selected job"""
        if self.selected_job_index is None:
            messagebox.showerror("Error", "No job selected")
            return
        try:
            job_details = self.get_job_details()
            self.backup_manager.update_job(self.selected_job_index, job_details)
            self.update_job_list()
            self.clear_form()
        except Exception as e:
            messagebox.showerror("Error", str(e))

    def delete_job(self):
        """Delete the selected job"""
        if self.selected_job_index is None:
            messagebox.showerror("Error", "No job selected")
            return
        job = self.backup_manager.active_jobs[self.selected_job_index]
        if askyesno("Confirm Delete", f"Are you sure you want to delete job: {job['zip_name']}?"):
            try:
                self.backup_manager.delete_job(self.selected_job_index)
                self.selected_job_index = None
                self.clear_form()
                self.update_job_list()
            except Exception as e:
                self.logger.error(f"Error removing job: {str(e)}")
                messagebox.showerror("Error", str(e))

    def show_monthly_status(self):
        """Show monthly backup status"""
        try:
            self.backup_manager.show_monthly_archive_status()
        except Exception as e:
            messagebox.showerror("Error", f"Could not show monthly status: {str(e)}")

    def run_monthly_backup(self):
        """Run monthly backup for all jobs"""
        try:
            # Ask user for monthly backup destination
            initial_dir = (self.backup_manager.monthly_backup_destination or 
                          r"C:\arkade\Arkade Shared Global\FOTM Backups")
            
            monthly_dest = filedialog.askdirectory(
                title="Select Monthly Backup Destination",
                initialdir=initial_dir
            )
            
            if monthly_dest:
                # Save the destination for future use
                self.backup_manager.set_monthly_backup_destination(monthly_dest)
                self.backup_manager.archive_monthly_backups(monthly_dest)
            else:
                # Use default destination
                self.backup_manager.archive_monthly_backups()
                
        except Exception as e:
            messagebox.showerror("Error", f"Could not run monthly backup: {str(e)}")

    def run_selected_job(self):
        """Run the selected job immediately"""
        if self.selected_job_index is None:
            messagebox.showerror("Error", "No job selected")
            return
        job = self.backup_manager.active_jobs[self.selected_job_index].copy()
        job['is_scheduled'] = False  # Mark as manual backup
        try:
            # Queue the backup for sequential execution
            self.backup_manager._queue_backup(job)
            # UI list will update when files appear on disk; you can also poll if desired
        except Exception as e:
            messagebox.showerror("Error", str(e))

    def clear_form(self):
        """Clear all form fields"""
        self.source_var.set("")
        self.dest_var.set("")
        self.zip_name_var.set("")
        self.interval_value_var.set("")
        self.interval_unit_var.set("minutes")
        self.keep_days_var.set("20")
        self.selected_job_index = None
        self.selected_map_var.set("")
        self.include_config_files.set(0)
        self.include_save_files.set(0)
        self.include_server_config.set(0)

    def refresh_jobs(self):
        """Manually refresh the job list"""
        # Debug logging removed as requested
        
        # Force reload jobs from the backup manager
        if hasattr(self.backup_manager, 'load_jobs'):
            self.backup_manager.load_jobs()
            # Debug logging removed as requested
        
        self._try_load_jobs()
        self.update_job_list()
        
        # Debug logging removed as requested

    def initialize_job_list(self):
        """Initialize the job list - called by backup manager when it's ready"""
        # Debug logging removed as requested
        
        # Force reload jobs from the backup manager
        if hasattr(self.backup_manager, 'load_jobs'):
            self.backup_manager.load_jobs()
            # Debug logging removed as requested
        
        self._try_load_jobs()
        self.update_job_list()

    def update_theme(self):
        """Update theme for all widgets in the job frame"""
        colors = ModernTheme.get_colors(self.theme_var.get())

        # Update Treeview colors
        style = ttk.Style()
        style.configure("Treeview",
            background=colors['surface'],
            foreground=colors['text_primary'],
            fieldbackground=colors['surface']
        )
        style.configure("Treeview.Heading",
            background=colors['header'],
            foreground=colors['text_primary']
        )
        style.map("Treeview",
            background=[('selected', colors['highlight'])],
            foreground=[('selected', colors['button_fg'])]
        )
        
        # Update status label colors
        if hasattr(self, 'status_label'):
            if self.theme_var.get() == "dark":
                self.status_label.config(background=colors['bg'], foreground=colors['fg'])
            else:
                self.status_label.config(background=colors['bg'], foreground=colors['fg'])
                
        # Update button colors
        self.update_button_colors()

    def get_button_color(self):
        """Get button background color based on current theme"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        return colors['button_bg']
        
    def get_button_text_color(self):
        """Get button text color based on current theme"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        return colors['button_fg']
        
    def get_button_hover_color(self):
        """Get button hover color based on current theme"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        return colors['hover']
        
    def update_button_colors(self):
        """Update colors of all action buttons"""
        try:
            # Recursively find and update all tk.Button widgets
            def update_buttons_recursive(widget):
                if isinstance(widget, tk.Button):
                    widget.config(
                        bg=self.get_button_color(),
                        fg=self.get_button_text_color(),
                        activebackground=self.get_button_hover_color(),
                        activeforeground=self.get_button_text_color()
                    )
                elif hasattr(widget, 'winfo_children'):
                    for child in widget.winfo_children():
                        update_buttons_recursive(child)
            
            # Start from the root widget
            update_buttons_recursive(self)
        except Exception as e:
            print(f"Error updating button colors: {e}")

    def load_job_details(self, job):
        """Load job details into the form"""
        # Load basic details
        self.source_var.set(job['source'])
        self.dest_var.set(job['destination'])
        self.zip_name_var.set(job['zip_name'])
        self.interval_value_var.set(job['interval_value'])
        self.interval_unit_var.set(job['interval_unit'])
        self.keep_days_var.set(str(job['keep_days']))

        # Load game-specific details
        if job['game_type'] == 'ARK':
            # Load ARK-specific details
            if 'selected_map' in job:
                self.selected_map_var.set(job['selected_map'])

            # Load checkboxes
            self.include_config_files.set(1 if job.get('include_config', False) else 0)
            self.include_save_files.set(1 if job.get('include_save', False) else 0)
            self.include_server_config.set(1 if job.get('include_server_config', False) else 0)

    def get_job_details(self):
        """Collect all job details from form"""
        # Get common details
        job_details = {
            'game_type': self.game_type_var.get(),
            'source': self.source_var.get().strip(),
            'destination': self.dest_var.get().strip(),
            'zip_name': self.zip_name_var.get().strip(),
            'interval_value': self.interval_value_var.get().strip(),
            'interval_unit': self.interval_unit_var.get(),
            'keep_days': int(self.keep_days_var.get().strip())
        }

        # Add game-specific details
        if job_details['game_type'] == "ARK":
            job_details.update({
                'selected_map': self.selected_map_var.get(),
                'include_config': bool(self.include_config_files.get()),
                'include_save': bool(self.include_save_files.get()),
                'include_server_config': bool(self.include_server_config.get())
            })

            # Validate ARK-specific settings
            if not job_details['selected_map']:
                raise ValueError("Please select a map for ARK backup")

        else:  # Palworld
            job_details['save_type'] = 'palworld'

        return job_details

    def on_game_type_changed(self, event):
        """Handle game type change"""
        # Clear game-specific frame
        for widget in self.game_specific_frame.winfo_children():
            widget.destroy()

        if self.game_type_var.get() == "ARK":
            self.create_ark_options()
        else:
            self.create_palworld_options()

    def create_ark_options(self):
        """Create ARK-specific options"""
        # ARK Options Frame
        ark_frame = ttk.LabelFrame(self.game_specific_frame, text="ARK Options")
        ark_frame.pack(fill='x', padx=5, pady=5)

        # Map Selection
        map_frame = ttk.Frame(ark_frame)
        map_frame.pack(fill='x', padx=5, pady=5)
        ttk.Label(map_frame, text="Select Map:").pack(side='left')
        map_dropdown = ttk.Combobox(
            map_frame,
            textvariable=self.selected_map_var,
            values=self.map_options,
            state="readonly"
        )
        map_dropdown.pack(side='left', padx=5)

        # Backup Options Frame
        backup_options_frame = ttk.LabelFrame(self.game_specific_frame, text="Backup Options")
        backup_options_frame.pack(fill='x', padx=5, pady=5)

        # Player and Map Saves
        CheckboxWithSymbol(
            backup_options_frame,
            text="Player Saves and Map Save",
            variable=self.include_save_files
        ).pack(fill='x', padx=5, pady=2)

        # Server Files
        CheckboxWithSymbol(
            backup_options_frame,
            text="Server Files (GameUserSettings.ini, Game.ini)",
            variable=self.include_server_config
        ).pack(fill='x', padx=5, pady=2)

        # Plugin Config Files
        CheckboxWithSymbol(
            backup_options_frame,
            text="Plugin Config Files",
            variable=self.include_config_files
        ).pack(fill='x', padx=5, pady=2)

        # Add Backup Settings
        self.create_backup_settings(self.game_specific_frame)

    def create_palworld_options(self):
        """Create Palworld-specific options"""
        # Only create backup settings for Palworld
        self.create_backup_settings(self.game_specific_frame)

    def create_backup_settings(self, parent):
        """Create common backup settings"""
        settings_frame = ttk.LabelFrame(parent, text="Backup Settings")
        settings_frame.pack(fill='x', padx=5, pady=5)

        # Backup Name
        name_frame = ttk.Frame(settings_frame)
        name_frame.pack(fill='x', padx=5, pady=5)
        ttk.Label(name_frame, text="Backup Name:").pack(side='left')
        ttk.Entry(name_frame, textvariable=self.zip_name_var).pack(side='left', fill='x', expand=True, padx=5)

        # Interval settings
        interval_frame = ttk.Frame(settings_frame)
        interval_frame.pack(fill='x', padx=5, pady=5)
        ttk.Label(interval_frame, text="Run Every:").pack(side='left')
        ttk.Entry(interval_frame, textvariable=self.interval_value_var, width=10).pack(side='left', padx=5)
        ttk.Combobox(
            interval_frame,
            textvariable=self.interval_unit_var,
            values=["minutes", "hours", "days"],
            state="readonly",
            width=10
        ).pack(side='left')

        # Keep days setting
        keep_frame = ttk.Frame(settings_frame)
        keep_frame.pack(fill='x', padx=5, pady=5)
        ttk.Label(keep_frame, text="Keep Backups For (days):").pack(side='left')
        ttk.Entry(keep_frame, textvariable=self.keep_days_var, width=10).pack(side='left', padx=5)


class BackupsTab(JobFrame):
    pass
