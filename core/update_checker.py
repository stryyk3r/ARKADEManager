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
            # Get the actual executable/script path
            if getattr(sys, 'frozen', False):
                # Running as compiled executable
                current_dir = os.path.dirname(sys.executable)
            else:
                # Running as script
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
            
            # Copy files to current directory
            if progress_callback:
                progress_callback("Installing update files...", 50)
            
            self._copy_update_files(source_dir, current_dir)
            self.log("Update files copied successfully")
            
            # Build the EXE if running as executable or if EXE exists
            exe_path = os.path.join(current_dir, "ArkadeManager.exe")
            spec_path = os.path.join(current_dir, "ArkadeManager.spec")
            if os.path.exists(spec_path) and (getattr(sys, 'frozen', False) or os.path.exists(exe_path)):
                if progress_callback:
                    progress_callback("Rebuilding EXE...", 70)
                
                self.log("Rebuilding EXE with updated code...")
                if self._build_exe(current_dir, progress_callback):
                    self.log("EXE rebuilt successfully")
                else:
                    self.log("Warning: EXE rebuild failed, but update files are installed")
            
            if progress_callback:
                progress_callback("Update complete! Restarting...", 95)
            
            self.log("Update completed successfully. Restarting application...")
            
            # Restart the application
            self._restart_application(current_dir)
            
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
            # Determine how to restart the application
            if getattr(sys, 'frozen', False):
                # Running as compiled executable
                restart_command = f'start "" "{sys.executable}"'
                app_name = "ArkadeManager.exe"
            else:
                # Running as script
                main_py = os.path.join(current_dir, 'main.py')
                restart_command = f'start "" python "{main_py}"'
                app_name = "main.py"
            
            if sys.platform.startswith('win'):
                # Windows batch script - use quotes for paths with spaces
                script_content = f'''@echo off
setlocal enabledelayedexpansion
echo Waiting for application to close...
timeout /t 5 /nobreak >nul

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
                    # Skip backup_jobs.json to preserve user data during updates
                    if item == 'backup_jobs.json':
                        continue
                    # Skip dist folder to prevent nesting
                    if item == 'dist':
                        continue
                    # Skip build folder
                    if item == 'build':
                        continue
                    
                    source_item = os.path.join(source_dir, item)
                    # Escape paths with quotes for safety
                    source_quoted = f'"{source_item}"'
                    dest_quoted = f'"{item}"'
                    
                    if os.path.isdir(source_item):
                        script_content += f'if exist {dest_quoted} rmdir /s /q {dest_quoted}\n'
                        script_content += f'xcopy {source_quoted} {dest_quoted} /e /i /y /q\n'
                    else:
                        if not item.startswith('.') or item in ['requirements.txt', 'README.md']:
                            script_content += f'copy {source_quoted} {dest_quoted} /y\n'
                
                script_content += f'''
echo Cleaning up temporary files...
if exist "{temp_dir}" rmdir /s /q "{temp_dir}"

echo Update completed! Starting application...
timeout /t 2 /nobreak >nul
{restart_command}

echo Update script completed.
del "%~f0"
'''
                script_ext = '.bat'
            else:
                # Unix shell script
                script_content = f'''#!/bin/bash
echo "Waiting for application to close..."
sleep 5

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
                    # Skip backup_jobs.json to preserve user data during updates
                    if item == 'backup_jobs.json':
                        continue
                    # Skip dist folder to prevent nesting
                    if item == 'dist':
                        continue
                    # Skip build folder
                    if item == 'build':
                        continue
                    
                    source_item = os.path.join(source_dir, item)
                    if os.path.isdir(source_item):
                        script_content += f'rm -rf "{item}"\n'
                        script_content += f'cp -r "{source_item}" "{item}"\n'
                    else:
                        if not item.startswith('.') or item in ['requirements.txt', 'README.md']:
                            script_content += f'cp "{source_item}" "{item}"\n'
                
                # Determine restart command for Unix
                if getattr(sys, 'frozen', False):
                    unix_restart = f'"{sys.executable}" &'
                else:
                    main_py = os.path.join(current_dir, 'main.py')
                    unix_restart = f'python3 "{main_py}" &'
                
                script_content += f'''
echo "Cleaning up temporary files..."
rm -rf "{temp_dir}"

echo "Update completed! Starting application..."
sleep 2
{unix_restart}

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
    
    def _copy_update_files(self, source_dir, current_dir):
        """Copy update files from source to current directory"""
        try:
            items_to_skip = ['.git', '__pycache__', '.gitignore', '.gitattributes', 
                           'backup_before_update', 'dist', 'build', 'backup_jobs.json']
            
            for item in os.listdir(source_dir):
                if item in items_to_skip or item.startswith('backup_'):
                    continue
                
                source_item = os.path.join(source_dir, item)
                dest_item = os.path.join(current_dir, item)
                
                if os.path.isdir(source_item):
                    # Remove existing directory and copy new one
                    if os.path.exists(dest_item):
                        shutil.rmtree(dest_item)
                    shutil.copytree(source_item, dest_item)
                    self.log(f"Copied directory: {item}")
                else:
                    # Copy file
                    if not item.startswith('.') or item in ['requirements.txt', 'README.md', '.gitignore']:
                        shutil.copy2(source_item, dest_item)
                        self.log(f"Copied file: {item}")
            
            return True
        except Exception as e:
            self.log(f"Error copying update files: {str(e)}")
            raise e
    
    def _build_exe(self, current_dir, progress_callback=None):
        """Build the EXE using PyInstaller"""
        try:
            spec_path = os.path.join(current_dir, "ArkadeManager.spec")
            if not os.path.exists(spec_path):
                self.log("No spec file found, skipping EXE build")
                return False
            
            # Check if PyInstaller is available
            try:
                result = subprocess.run(
                    [sys.executable, "-m", "pip", "show", "pyinstaller"],
                    capture_output=True,
                    check=False
                )
                if result.returncode != 0:
                    self.log("PyInstaller not found, installing...")
                    if progress_callback:
                        progress_callback("Installing PyInstaller...", 72)
                    subprocess.run(
                        [sys.executable, "-m", "pip", "install", "--quiet", "pyinstaller"],
                        check=True
                    )
            except Exception as e:
                self.log(f"Error checking/installing PyInstaller: {str(e)}")
                return False
            
            # Clean previous build
            build_dir = os.path.join(current_dir, "build")
            dist_dir = os.path.join(current_dir, "dist")
            if os.path.exists(build_dir):
                shutil.rmtree(build_dir)
            if os.path.exists(dist_dir):
                shutil.rmtree(dist_dir)
            
            if progress_callback:
                progress_callback("Building EXE...", 75)
            
            # Build the EXE
            self.log("Running PyInstaller...")
            result = subprocess.run(
                [sys.executable, "-m", "PyInstaller", "--clean", "--noconfirm", spec_path],
                cwd=current_dir,
                capture_output=True,
                text=True,
                check=False
            )
            
            if result.returncode != 0:
                self.log(f"PyInstaller build failed: {result.stderr}")
                return False
            
            # Copy the built EXE to main directory
            built_exe = os.path.join(dist_dir, "ArkadeManager.exe")
            if os.path.exists(built_exe):
                exe_dest = os.path.join(current_dir, "ArkadeManager.exe")
                # Backup old EXE
                if os.path.exists(exe_dest):
                    backup_exe = exe_dest + ".old"
                    if os.path.exists(backup_exe):
                        os.remove(backup_exe)
                    os.rename(exe_dest, backup_exe)
                
                shutil.copy2(built_exe, exe_dest)
                self.log("EXE built and deployed successfully")
                return True
            else:
                self.log("Built EXE not found in dist folder")
                return False
                
        except Exception as e:
            self.log(f"Error building EXE: {str(e)}")
            return False
    
    def _restart_application(self, current_dir):
        """Restart the application"""
        try:
            self.log("Restarting application...")
            
            # Determine how to restart
            if getattr(sys, 'frozen', False):
                # Running as executable - restart the EXE
                exe_path = os.path.join(current_dir, "ArkadeManager.exe")
                if os.path.exists(exe_path):
                    restart_cmd = [exe_path]
                else:
                    restart_cmd = [sys.executable]
            else:
                # Running as script - restart with Python
                main_py = os.path.join(current_dir, "main.py")
                restart_cmd = [sys.executable, main_py]
            
            # Start the new process
            if sys.platform.startswith('win'):
                # On Windows, use CREATE_NEW_CONSOLE to start in new window
                subprocess.Popen(
                    restart_cmd,
                    cwd=current_dir,
                    creationflags=subprocess.CREATE_NEW_CONSOLE | subprocess.DETACHED_PROCESS
                )
            else:
                # On Unix, start in background
                subprocess.Popen(restart_cmd, cwd=current_dir, start_new_session=True)
            
            # Give it a moment to start
            import time
            time.sleep(1)
            
            # Close this instance
            os._exit(0)
            
        except Exception as e:
            self.log(f"Error restarting application: {str(e)}")
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
    

