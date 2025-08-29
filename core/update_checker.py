import requests
import json
import os
import zipfile
import shutil
import subprocess
import sys
import tempfile
import threading
from tkinter import messagebox, ttk
from datetime import datetime

class UpdateChecker:
    def __init__(self, current_version, check_url):
        self.current_version = current_version
        self.check_url = check_url
        self.logger = None
        
    def set_logger(self, logger):
        """Set logger for update operations"""
        self.logger = logger
        
    def log(self, message):
        """Log message if logger is available"""
        if self.logger:
            self.logger.info(f"[Update Checker] {message}")
        else:
            print(f"[Update Checker] {message}")
            
    def check_for_updates(self):
        """Check for available updates"""
        try:
            self.log("Checking for updates...")
            response = requests.get(self.check_url, timeout=10)
            
            if response.status_code == 200:
                update_info = response.json()
                return self._compare_versions(update_info)
            else:
                self.log(f"Failed to check for updates: HTTP {response.status_code}")
                return None
                
        except requests.exceptions.Timeout:
            self.log("Update check timed out")
            return None
        except requests.exceptions.RequestException as e:
            self.log(f"Network error during update check: {str(e)}")
            return None
        except Exception as e:
            self.log(f"Error checking for updates: {str(e)}")
            return None
            
    def _compare_versions(self, update_info):
        """Compare current version with available version"""
        try:
            available_version = update_info.get('tag_name', '').lstrip('v')
            current_tuple = self._version_to_tuple(self.current_version)
            available_tuple = self._version_to_tuple(available_version)
            
            self.log(f"Current version: {self.current_version}")
            self.log(f"Available version: {available_version}")
            
            if available_tuple > current_tuple:
                self.log(f"Update available: {available_version}")
                # Get the download URL from assets
                download_url = self._get_download_url(update_info)
                
                return {
                    'version': available_version,
                    'download_url': download_url,
                    'release_url': update_info.get('html_url', ''),
                    'changelog': update_info.get('body', 'No changelog available'),
                    'release_date': update_info.get('published_at', ''),
                    'download_count': self._get_download_count(update_info)
                }
            else:
                self.log("No updates available")
                return None
                
        except Exception as e:
            self.log(f"Error comparing versions: {str(e)}")
            return None
            
    def _version_to_tuple(self, version_string):
        """Convert version string to tuple for comparison"""
        try:
            # Remove 'v' prefix if present
            version_string = version_string.lstrip('v')
            
            # Split by dots and convert to integers
            parts = version_string.split('.')
            version_tuple = tuple(int(part) for part in parts)
            
            return version_tuple
        except Exception as e:
            self.log(f"Error parsing version '{version_string}': {str(e)}")
            return (0, 0, 0)
            
    def get_version_info(self):
        """Get current version information"""
        return {
            'current_version': self.current_version,
            'check_url': self.check_url,
            'last_check': getattr(self, '_last_check_time', None)
        }
        
    def _get_download_url(self, update_info):
        """Get the actual download URL from GitHub release assets"""
        try:
            assets = update_info.get('assets', [])
            for asset in assets:
                if asset['name'].endswith('.zip'):
                    return asset['browser_download_url']
            
            # Fallback to release URL if no assets found
            return update_info.get('html_url', '')
            
        except Exception as e:
            self.log(f"Error getting download URL: {str(e)}")
            return update_info.get('html_url', '')
            
    def _get_download_count(self, update_info):
        """Get download count from assets"""
        try:
            assets = update_info.get('assets', [])
            if assets:
                return assets[0].get('download_count', 0)
            return 0
        except Exception as e:
            self.log(f"Error getting download count: {str(e)}")
            return 0

    def download_and_install_update(self, update_info, progress_callback=None):
        """Download and install the update"""
        temp_dir = None
        try:
            self.log("Starting update download and installation...")
            
            # Validate update info
            if not update_info or not isinstance(update_info, dict):
                raise Exception("Invalid update information provided")
            
            # Get the download URL
            download_url = update_info.get('download_url')
            if not download_url or download_url == update_info.get('release_url'):
                raise Exception("No direct download URL available. Please download manually from the release page.")
            
            # Validate that we have write permissions to the current directory
            current_dir = os.path.dirname(os.path.abspath(sys.argv[0]))
            if not os.access(current_dir, os.W_OK):
                raise Exception("No write permission to application directory. Please run as administrator.")
            
            # Create temporary directory for download
            temp_dir = tempfile.mkdtemp()
            zip_path = os.path.join(temp_dir, "update.zip")
            
            # Download the update
            self.log(f"Downloading update from: {download_url}")
            if progress_callback:
                progress_callback("Downloading update...", 0)
            
            response = requests.get(download_url, stream=True, timeout=30)
            response.raise_for_status()
            
            total_size = int(response.headers.get('content-length', 0))
            downloaded_size = 0
            
            with open(zip_path, 'wb') as f:
                for chunk in response.iter_content(chunk_size=8192):
                    if chunk:
                        f.write(chunk)
                        downloaded_size += len(chunk)
                        if total_size > 0 and progress_callback:
                            progress = int((downloaded_size / total_size) * 50)  # First 50% for download
                            progress_callback(f"Downloading... {downloaded_size}/{total_size} bytes", progress)
            
            self.log("Download completed successfully")
            
            # Validate the downloaded file
            if not os.path.exists(zip_path) or os.path.getsize(zip_path) == 0:
                raise Exception("Downloaded file is empty or corrupted")
            
            # Extract the update
            if progress_callback:
                progress_callback("Extracting update...", 50)
            
            extract_dir = os.path.join(temp_dir, "extracted")
            os.makedirs(extract_dir, exist_ok=True)
            
            try:
                with zipfile.ZipFile(zip_path, 'r') as zip_ref:
                    zip_ref.extractall(extract_dir)
            except zipfile.BadZipFile:
                raise Exception("Downloaded file is not a valid ZIP archive")
            
            self.log("Update extracted successfully")
            
            # Find the main application directory
            if progress_callback:
                progress_callback("Preparing installation...", 75)
            
            # Create backup of current version
            backup_dir = os.path.join(current_dir, "backup_before_update")
            if os.path.exists(backup_dir):
                shutil.rmtree(backup_dir)
            shutil.copytree(current_dir, backup_dir, ignore=shutil.ignore_patterns('backup_before_update', '__pycache__', '*.pyc'))
            
            self.log("Backup created successfully")
            
            # Copy new files
            if progress_callback:
                progress_callback("Installing update...", 90)
            
            # Find the extracted application files
            extracted_files = os.listdir(extract_dir)
            if len(extracted_files) == 1 and os.path.isdir(os.path.join(extract_dir, extracted_files[0])):
                # If there's a single directory, use its contents
                source_dir = os.path.join(extract_dir, extracted_files[0])
            else:
                # Use the extract directory directly
                source_dir = extract_dir
            
            # Validate that we have the expected files
            if not os.path.exists(os.path.join(source_dir, 'main.py')):
                raise Exception("Invalid update package: main.py not found")
            
            # Copy files from source to current directory
            for item in os.listdir(source_dir):
                source_item = os.path.join(source_dir, item)
                dest_item = os.path.join(current_dir, item)
                
                if os.path.isdir(source_item):
                    if os.path.exists(dest_item):
                        shutil.rmtree(dest_item)
                    shutil.copytree(source_item, dest_item)
                else:
                    shutil.copy2(source_item, dest_item)
            
            self.log("Update installed successfully")
            
            # Clean up temporary files
            if temp_dir and os.path.exists(temp_dir):
                shutil.rmtree(temp_dir)
            
            if progress_callback:
                progress_callback("Update completed! Restarting application...", 100)
            
            self.log("Update completed successfully. Restarting application...")
            
            # Restart the application
            self._restart_application()
            
        except Exception as e:
            self.log(f"Error during update: {str(e)}")
            # Clean up temporary files if they exist
            try:
                if temp_dir and os.path.exists(temp_dir):
                    shutil.rmtree(temp_dir)
            except:
                pass
            raise e
    
    def _check_admin_privileges(self):
        """Check if the application is running with administrator privileges"""
        try:
            if sys.platform.startswith('win'):
                import ctypes
                return ctypes.windll.shell32.IsUserAnAdmin() != 0
            else:
                # On Unix-like systems, check if we can write to system directories
                return os.geteuid() == 0
        except:
            return False
    
    def _restart_application(self):
        """Restart the application"""
        try:
            # Get the current script path
            script_path = sys.argv[0]
            
            # Check if we need admin privileges
            needs_admin = not os.access(os.path.dirname(os.path.abspath(script_path)), os.W_OK)
            
            # Create restart script
            if sys.platform.startswith('win'):
                # Windows
                if needs_admin:
                    restart_script = f'''@echo off
timeout /t 2 /nobreak >nul
powershell -Command "Start-Process '{script_path}' -Verb RunAs"
exit
'''
                else:
                    restart_script = f'''@echo off
timeout /t 2 /nobreak >nul
start "" "{script_path}"
exit
'''
                script_ext = '.bat'
            else:
                # Unix-like systems
                if needs_admin:
                    restart_script = f'''#!/bin/bash
sleep 2
sudo python3 "{script_path}" &
exit
'''
                else:
                    restart_script = f'''#!/bin/bash
sleep 2
python3 "{script_path}" &
exit
'''
                script_ext = '.sh'
            
            # Write restart script to temporary file
            temp_script = tempfile.NamedTemporaryFile(mode='w', suffix=script_ext, delete=False)
            temp_script.write(restart_script)
            temp_script.close()
            
            # Make script executable on Unix systems
            if not sys.platform.startswith('win'):
                os.chmod(temp_script.name, 0o755)
            
            # Execute restart script
            subprocess.Popen([temp_script.name], shell=True)
            
            # Exit current application
            sys.exit(0)
            
        except Exception as e:
            self.log(f"Error restarting application: {str(e)}")
            messagebox.showinfo("Update Complete", 
                "Update completed successfully!\n\n"
                "Please restart the application manually to apply the changes.")
