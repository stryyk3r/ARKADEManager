import tkinter as tk
from tkinter import ttk, messagebox
import threading
import os, sys, ctypes
from pathlib import Path

# Give the app its own identity on the Windows taskbar (prevents grouping under Python)
try:
    ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID("com.arkade.manager")
except Exception:
    pass

def resource_path(rel: str) -> str:
    """Return absolute path to resource, works for dev & PyInstaller onefile."""
    base = getattr(sys, "_MEIPASS", None)
    if base:
        return str(Path(base) / rel)   # temp dir used by onefile
    return str(Path(__file__).parent / rel)

from tabs.tab_backups import BackupsTab
from tabs.tab_logs import LogsTab
from tabs.tab_plugins import PluginsTab
from tabs.tab_game_ini import GameIniTab
from tabs.tab_gus_ini import GUSIniTab
from tabs.tab_tribefile import TribeFileTab
from tabs.tab_plugintoggle import PluginToggleTab
from tabs.tab_newIP import IPUpdaterTab
from tabs.tab_plugin_manager import PluginManagerTab

from core.backup_core import BackupManager
from core.overdue_patch import apply_overdue_patch   # <-- added
from core.logger import Logger
from core.theme import ModernTheme
from core.update_checker import UpdateChecker

# Apply the overdue/last-save patch to BackupManager BEFORE creating the app
apply_overdue_patch(BackupManager)

# Version information
VERSION = "1.0.19"
UPDATE_CHECK_URL = "https://api.github.com/repos/stryyk3r/ARKADEManager/releases/latest"


class ArkadeManagerApp(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title(f"Arkade Manager v{VERSION}")
        self.geometry("1100x800")
        
        # Set custom icon (.ico preferred on Windows)
        try:
            ico = resource_path("arkade_icon.ico")
            self.iconbitmap(ico)
        except Exception as e:
            print(f"ICO failed ({e}); trying PNG fallback")
            try:
                from tkinter import PhotoImage
                png = resource_path("arkade_logo.png")
                self.iconphoto(False, PhotoImage(file=png))
            except Exception as e2:
                print(f"PNG fallback failed: {e2}")
        
        self.theme_var = tk.StringVar(value="dark")
        
        # Create theme toggle button
        self.create_theme_toggle()

        self.logger = Logger()
        self.backup_manager = BackupManager()
        
        # Initialize update checker
        self.update_checker = UpdateChecker(VERSION, UPDATE_CHECK_URL)
        self.update_checker.set_logger(self.logger)

        self.notebook = ttk.Notebook(self)
        self.notebook.pack(fill="both", expand=True)

        self.backups_tab = BackupsTab(self.notebook, self.theme_var, self.backup_manager, self.logger)
        self.logs_tab    = LogsTab(self.notebook, self.theme_var)
        self.plugins_tab = PluginsTab(self.notebook, self.theme_var)
        self.game_tab    = GameIniTab(self.notebook, self.theme_var)
        self.gus_tab     = GUSIniTab(self.notebook, self.theme_var)
        self.tribe_tab   = TribeFileTab(self.notebook, self.theme_var)
        self.plugin_toggle_tab = PluginToggleTab(self.notebook, self.theme_var)
        self.ip_updater_tab = IPUpdaterTab(self.notebook, self.theme_var)
        self.plugin_manager_tab = PluginManagerTab(self.notebook, self.theme_var, self.logger)

        self.notebook.add(self.backups_tab, text="Backups")
        self.notebook.add(self.logs_tab, text="Logs")
        self.notebook.add(self.plugins_tab, text="New Plugins")
        self.notebook.add(self.game_tab, text="Game.ini")
        self.notebook.add(self.gus_tab, text="GameUserSettings.ini")
        self.notebook.add(self.tribe_tab, text="Tribe Files")
        self.notebook.add(self.plugin_toggle_tab, text=".ini Editor")
        self.notebook.add(self.ip_updater_tab, text="IP Updater")
        self.notebook.add(self.plugin_manager_tab, text="Plugin Manager")

        # Wire the logger to the logs tab
        if hasattr(self.logger, "set_callback"):
            self.logger.set_callback(self.logs_tab.add_message, ui_thread_call=self.logs_tab.ui_call)
            # Test the logger connection
            # self.logger.info("Logger connected successfully")

        # Let BackupManager reference the job frame if it uses it
        if hasattr(self.backup_manager, "set_job_frame"):
            self.backup_manager.set_job_frame(self.backups_tab)
            # self.logger.info("Backup manager job frame set")
        
        # Run startup overdue check once (not through scheduler)
        if hasattr(self.backup_manager, "run_startup_overdue_check"):
            self.backup_manager.run_startup_overdue_check()
        
        # Apply initial theme
        ModernTheme.apply_theme(self, self.theme_var)
        


        # periodic tick - run every 30 seconds (optimized with smart scheduler)
        self.after(30000, self._tick)
        
        # Handle application shutdown
        self.protocol("WM_DELETE_WINDOW", self._on_closing)

    def _on_closing(self):
        """Handle application shutdown"""
        try:
            # Shutdown backup manager
            if hasattr(self.backup_manager, "shutdown"):
                self.backup_manager.shutdown()
        except Exception as e:
            print(f"Error during shutdown: {e}")
        finally:
            # Destroy the window
            self.destroy()

    def _tick(self):
        try:
            # Run the scheduler (which includes both scheduled jobs and overdue checks)
            if hasattr(self.backup_manager, "run_scheduler"):
                # Add debug logging to see if scheduler is being called
                if not hasattr(self, '_tick_debug_logged'):
                    self.logger.info("Scheduler tick started - running every 30 seconds")
                    self._tick_debug_logged = True
                
                self.backup_manager.run_scheduler()

            # Throttle UI refresh to every ~30 seconds to avoid constant repopulation
            now = getattr(self, "_last_jobs_refresh", 0)
            import time
            if time.time() - now >= 30.0:
                if hasattr(self.backups_tab, "update_job_list"):
                    self.backups_tab.update_job_list()
                self._last_jobs_refresh = time.time()
        finally:
            # keep the core tick at 30s (optimized with smart scheduler)
            self.after(30000, self._tick)

    def create_theme_toggle(self):
        """Create theme toggle button and update checker"""
        # Create a frame for the theme toggle and update button
        theme_frame = ttk.Frame(self)
        theme_frame.pack(side='top', fill='x', padx=5, pady=2)
        
        # Create update check button
        self.update_button = tk.Button(
            theme_frame,
            text="🔄 Check for Updates",
            command=self.check_for_updates,
            relief="raised",
            borderwidth=3,
            bg=self.get_theme_button_color(),
            fg=self.get_theme_button_text_color(),
            activebackground=self.get_theme_button_hover_color(),
            activeforeground=self.get_theme_button_text_color()
        )
        self.update_button.pack(side='right', padx=5)
        
        # Create theme toggle button with beveled edges
        self.theme_button = tk.Button(
            theme_frame,
            text="🌙 Dark Mode",
            command=self.toggle_theme,
            relief="raised",
            borderwidth=3,
            bg=self.get_theme_button_color(),
            fg=self.get_theme_button_text_color(),
            activebackground=self.get_theme_button_hover_color(),
            activeforeground=self.get_theme_button_text_color()
        )
        self.theme_button.pack(side='right', padx=5)
        
        # Add easter egg hover functionality
        self._setup_theme_button_easter_egg()
        
        # Update button text based on current theme
        self.update_theme_button_text()

    def toggle_theme(self):
        """Toggle between light and dark themes"""
        current_theme = self.theme_var.get()
        new_theme = "light" if current_theme == "dark" else "dark"
        self.theme_var.set(new_theme)
        
        # Apply the new theme
        ModernTheme.apply_theme(self, self.theme_var)
        
        # Force refresh of all widgets
        self.update_idletasks()
        
        # Update all tabs with the new theme
        self.update_all_tabs_theme()
        
        # Force another refresh after tab updates
        self.update_idletasks()
        
        # Update button text
        self.update_theme_button_text()

    def update_theme_button_text(self):
        """Update the theme button text based on current theme"""
        current_theme = self.theme_var.get()
        if current_theme == "dark":
            self.theme_button.config(text="☀️ Light Mode")
        else:
            self.theme_button.config(text="🌙 Dark Mode")
        
        # Update button colors
        self.theme_button.config(
            bg=self.get_theme_button_color(),
            fg=self.get_theme_button_text_color(),
            activebackground=self.get_theme_button_hover_color(),
            activeforeground=self.get_theme_button_text_color()
        )

    def _setup_theme_button_easter_egg(self):
        """Setup the easter egg hover functionality for the theme button"""
        self._easter_egg_timer = None
        self._easter_egg_tooltip = None
        
        # Bind mouse enter and leave events
        self.theme_button.bind('<Enter>', self._on_theme_button_enter)
        self.theme_button.bind('<Leave>', self._on_theme_button_leave)

    def _on_theme_button_enter(self, event):
        """Handle mouse enter on theme button"""
        # Only show easter egg in light mode
        if self.theme_var.get() == "light":
            # Start timer for 1 second
            self._easter_egg_timer = self.after(1000, self._show_bobs_mode_tooltip)

    def _on_theme_button_leave(self, event):
        """Handle mouse leave on theme button"""
        # Cancel timer if mouse leaves before 1 second
        if self._easter_egg_timer:
            self.after_cancel(self._easter_egg_timer)
            self._easter_egg_timer = None
        
        # Hide tooltip if it's showing
        self._hide_bobs_mode_tooltip()

    def _show_bobs_mode_tooltip(self):
        """Show the 'Bob's mode!' tooltip"""
        if self._easter_egg_tooltip:
            self._easter_egg_tooltip.destroy()
        
        # Create tooltip window
        self._easter_egg_tooltip = tk.Toplevel(self)
        self._easter_egg_tooltip.title("")
        self._easter_egg_tooltip.geometry("+0+0")
        self._easter_egg_tooltip.overrideredirect(True)
        self._easter_egg_tooltip.attributes('-topmost', True)
        
        # Get button position
        button_x = self.theme_button.winfo_rootx()
        button_y = self.theme_button.winfo_rooty()
        button_width = self.theme_button.winfo_width()
        button_height = self.theme_button.winfo_height()
        
        # Position tooltip above the button
        tooltip_x = button_x + (button_width // 2) - 50
        tooltip_y = button_y - 40
        
        self._easter_egg_tooltip.geometry(f"100x30+{tooltip_x}+{tooltip_y}")
        
        # Create tooltip label
        tooltip_label = tk.Label(
            self._easter_egg_tooltip,
            text="Bob's mode!",
            font=("Arial", 10, "bold"),
            bg="#ffffcc",
            fg="#000000",
            relief="solid",
            borderwidth=1
        )
        tooltip_label.pack(fill='both', expand=True)
        
        # Auto-hide after 2 seconds
        self.after(2000, self._hide_bobs_mode_tooltip)

    def _hide_bobs_mode_tooltip(self):
        """Hide the 'Bob's mode!' tooltip"""
        if self._easter_egg_tooltip:
            self._easter_egg_tooltip.destroy()
            self._easter_egg_tooltip = None

    def check_for_updates(self):
        """Check for available updates"""
        try:
            # Disable the update button temporarily
            self.update_button.config(state='disabled', text="Checking...")
            self.update_idletasks()
            
            # Check for updates
            update_info = self.update_checker.check_for_updates()
            
            if update_info:
                # Show update dialog
                self._show_update_dialog(update_info)
            else:
                messagebox.showinfo("Update Check", "You are running the latest version!")
                
        except Exception as e:
            messagebox.showerror("Update Error", f"Failed to check for updates: {str(e)}")
        finally:
            # Re-enable the update button
            self.update_button.config(state='normal', text="🔄 Check for Updates")

    def _show_update_dialog(self, update_info):
        """Show update available dialog"""
        changelog = update_info.get('changelog', 'No changelog available')
        if len(changelog) > 500:
            changelog = changelog[:500] + "..."
            
        message = f"Update Available!\n\n"
        message += f"Current Version: {VERSION}\n"
        message += f"New Version: {update_info['version']}\n\n"
        message += f"Changelog:\n{changelog}\n\n"
        message += "Would you like to download and install this update?"
        
        result = messagebox.askyesno("Update Available", message)
        if result:
            self._download_and_install_update(update_info)

    def _download_and_install_update(self, update_info):
        """Download and install the update"""
        try:
            # Create progress dialog
            progress_window = tk.Toplevel(self)
            progress_window.title("Updating Arkade Manager")
            progress_window.geometry("400x150")
            progress_window.resizable(False, False)
            progress_window.transient(self)
            progress_window.grab_set()
            
            # Set icon for progress window
            try:
                progress_window.iconbitmap(resource_path("arkade_icon.ico"))
            except Exception:
                pass
            
            # Center the progress window
            progress_window.update_idletasks()
            x = (progress_window.winfo_screenwidth() // 2) - (400 // 2)
            y = (progress_window.winfo_screenheight() // 2) - (150 // 2)
            progress_window.geometry(f"400x150+{x}+{y}")
            
            # Progress label
            progress_label = tk.Label(progress_window, text="Preparing update...", font=("Arial", 10))
            progress_label.pack(pady=20)
            
            # Progress bar
            progress_bar = ttk.Progressbar(progress_window, length=350, mode='determinate')
            progress_bar.pack(pady=10)
            
            # Status label
            status_label = tk.Label(progress_window, text="", font=("Arial", 9))
            status_label.pack(pady=10)
            
            def update_progress(message, percentage):
                """Update progress dialog"""
                progress_label.config(text=message)
                progress_bar['value'] = percentage
                status_label.config(text=f"{percentage}%")
                progress_window.update_idletasks()
            
            def start_update():
                """Start the update process in a separate thread"""
                try:
                    self.update_checker.download_and_install_update(update_info, update_progress)
                except Exception as e:
                    # Show error in main thread
                    self.after(0, lambda: self._show_update_error(str(e), progress_window, update_info))
            
            # Start update in separate thread
            update_thread = threading.Thread(target=start_update, daemon=True)
            update_thread.start()
            
        except Exception as e:
            messagebox.showerror("Update Error", f"Failed to start update: {str(e)}")
    
    def _show_update_error(self, error_message, progress_window, update_info):
        """Show update error and close progress window"""
        progress_window.destroy()
        
        # Create a more helpful error message with manual installation instructions
        manual_instructions = f"""
MANUAL INSTALLATION INSTRUCTIONS:

1. Close Arkade Manager completely
2. Download the latest version from:
   {update_info['release_url']}
3. Extract the ZIP file to a temporary location
4. Copy all files from the extracted folder to your current Arkade Manager folder
   (Replace all files except backup_jobs.json to preserve your backup jobs)
5. Restart Arkade Manager

Current location: {os.path.dirname(os.path.abspath(sys.argv[0]))}
"""
        
        messagebox.showerror("Update Error", 
            f"Failed to download and install update automatically:\n\n{error_message}\n\n"
            f"{manual_instructions}")

    def get_theme_button_color(self):
        """Get theme button background color based on current theme"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        return colors['button_bg']
        
    def get_theme_button_text_color(self):
        """Get theme button text color based on current theme"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        return colors['button_fg']
        
    def get_theme_button_hover_color(self):
        """Get theme button hover color based on current theme"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        return colors['hover']

    def update_all_tabs_theme(self):
        """Update theme for all tabs"""
        # Update each tab's theme
        if hasattr(self.backups_tab, 'update_theme'):
            self.backups_tab.update_theme()
        if hasattr(self.logs_tab, 'update_theme'):
            self.logs_tab.update_theme()
        if hasattr(self.plugins_tab, 'update_theme'):
            self.plugins_tab.update_theme()
        if hasattr(self.game_tab, 'update_theme'):
            self.game_tab.update_theme()
        if hasattr(self.gus_tab, 'update_theme'):
            self.gus_tab.update_theme()
        if hasattr(self.tribe_tab, 'update_theme'):
            self.tribe_tab.update_theme()
        if hasattr(self.plugin_toggle_tab, 'update_theme'):
            self.plugin_toggle_tab.update_theme()
        if hasattr(self.ip_updater_tab, 'update_theme'):
            self.ip_updater_tab.update_theme()
        if hasattr(self.plugin_manager_tab, 'update_theme'):
            self.plugin_manager_tab.update_theme()
            
        # Force refresh of the notebook widget
        try:
            if hasattr(self, 'notebook'):
                self.notebook.update_idletasks()
                self.notebook.update()
        except:
            pass



if __name__ == "__main__":
    app = ArkadeManagerApp()
    app.mainloop()
