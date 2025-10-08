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
            
            # Clean up any existing backup directories
            backup_dir = os.path.join(current_dir, "backup_before_update")
            if os.path.exists(backup_dir):
                try:
                    shutil.rmtree(backup_dir)
                    self.log("Cleaned up existing backup directory")
                except Exception as e:
                    self.log(f"Warning: Could not clean up existing backup: {str(e)}")
            
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
                            progress = int((downloaded_size / total_size) * 30)  # First 30% for download
                            progress_callback(f"Downloading... {downloaded_size}/{total_size} bytes", progress)
            
            self.log("Download completed successfully")
            
            # Validate the downloaded file
            if not os.path.exists(zip_path) or os.path.getsize(zip_path) == 0:
                raise Exception("Downloaded file is empty or corrupted")
            
            # Extract the update
            if progress_callback:
                progress_callback("Extracting update...", 30)
            
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
                progress_callback("Preparing installation...", 50)
            
            # Create backup of current version
            backup_dir = os.path.join(current_dir, "backup_before_update")
            if os.path.exists(backup_dir):
                shutil.rmtree(backup_dir)
            shutil.copytree(current_dir, backup_dir, ignore=shutil.ignore_patterns(
                'backup_before_update', '__pycache__', '*.pyc', '*.pyo', 
                '.git', '.gitignore', '.gitattributes', '.DS_Store', 'Thumbs.db'
            ))
            
            self.log("Backup created successfully")
            
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
            
            # Log what files we're about to copy
            self.log(f"Source directory contents: {os.listdir(source_dir)}")
            self.log(f"Current directory: {current_dir}")
            
            # Create the update script that will run after the application closes
            if progress_callback:
                progress_callback("Preparing update script...", 70)
            
            update_script = self._create_update_script(source_dir, current_dir, temp_dir)
            
            if progress_callback:
                progress_callback("Update ready! Closing application...", 90)
            
            self.log("Update prepared successfully. Closing application for installation...")
            
            # Close the application and run the update script
            self._close_and_update(update_script)
            
        except Exception as e:
            self.log(f"Error during update: {str(e)}")
            # Clean up temporary files if they exist
            try:
                if temp_dir and os.path.exists(temp_dir):
                    shutil.rmtree(temp_dir)
            except:
                pass
            raise e
    
    def _create_update_script(self, source_dir, current_dir, temp_dir):
        """Create a script that will perform the update after the application closes"""
        try:
            if sys.platform.startswith('win'):
                # Windows batch script
                script_content = f'''@echo off
echo Waiting for application to close...
timeout /t 3 /nobreak >nul

echo Installing update...
cd /d "{current_dir}"

echo Copying new files...
'''
                # Add copy commands for each file/directory
                for item in os.listdir(source_dir):
                    if item in ['.git', '__pycache__', '.gitignore', '.gitattributes']:
                        continue
                    if item.startswith('backup_'):
                        continue
<<<<<<< HEAD
                    # Skip backup_jobs.json to preserve user data during updates
                    if item == 'backup_jobs.json':
=======
                    # Skip dist folder to prevent nesting
                    if item == 'dist':
>>>>>>> 031bd5de9c69624bafbc05ff1227e7e14418846b
                        continue
                    
                    source_item = os.path.join(source_dir, item)
                    if os.path.isdir(source_item):
                        script_content += f'if exist "{item}" rmdir /s /q "{item}"\n'
                        script_content += f'xcopy "{source_item}" "{item}" /e /i /y\n'
                    else:
                        if not item.startswith('.') or item in ['requirements.txt', 'README.md']:
                            script_content += f'copy "{source_item}" "{item}" /y\n'
                
                script_content += f'''
echo Cleaning up temporary files...
rmdir /s /q "{temp_dir}"

echo Update completed! Starting application...
timeout /t 2 /nobreak >nul
start "" "{os.path.join(current_dir, 'main.py')}"

echo Update script completed.
del "%~f0"
'''
                script_ext = '.bat'
            else:
                # Unix shell script
                script_content = f'''#!/bin/bash
echo "Waiting for application to close..."
sleep 3

echo "Installing update..."
cd "{current_dir}"

echo "Copying new files..."
'''
                # Add copy commands for each file/directory
                for item in os.listdir(source_dir):
                    if item in ['.git', '__pycache__', '.gitignore', '.gitattributes']:
                        continue
                    if item.startswith('backup_'):
                        continue
<<<<<<< HEAD
                    # Skip backup_jobs.json to preserve user data during updates
                    if item == 'backup_jobs.json':
=======
                    # Skip dist folder to prevent nesting
                    if item == 'dist':
>>>>>>> 031bd5de9c69624bafbc05ff1227e7e14418846b
                        continue
                    
                    source_item = os.path.join(source_dir, item)
                    if os.path.isdir(source_item):
                        script_content += f'rm -rf "{item}"\n'
                        script_content += f'cp -r "{source_item}" "{item}"\n'
                    else:
                        if not item.startswith('.') or item in ['requirements.txt', 'README.md']:
                            script_content += f'cp "{source_item}" "{item}"\n'
                
                script_content += f'''
echo "Cleaning up temporary files..."
rm -rf "{temp_dir}"

echo "Update completed! Starting application..."
sleep 2
python3 "{os.path.join(current_dir, 'main.py')}" &

echo "Update script completed."
rm -- "$0"
'''
                script_ext = '.sh'
            
            # Write the update script to a temporary file
            temp_script = tempfile.NamedTemporaryFile(mode='w', suffix=script_ext, delete=False)
            temp_script.write(script_content)
            temp_script.close()
            
            # Make script executable on Unix systems
            if not sys.platform.startswith('win'):
                os.chmod(temp_script.name, 0o755)
            
            return temp_script.name
            
        except Exception as e:
            self.log(f"Error creating update script: {str(e)}")
            raise e
    
    def _close_and_update(self, update_script):
        """Close the application and run the update script"""
        try:
            # Execute the update script
            subprocess.Popen([update_script], shell=True)
            
            # Close the application
            self.log("Closing application for update...")
            
            # Use a more forceful exit to ensure the application closes
            if hasattr(self, 'logger'):
                try:
                    self.logger.info("Application closing for update...")
                except:
                    pass
            
            # Force exit the application
            os._exit(0)
            
        except Exception as e:
            self.log(f"Error closing application: {str(e)}")
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
    

