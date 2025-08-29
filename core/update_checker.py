import requests
import json
import os
import zipfile
import shutil
import subprocess
import sys
from tkinter import messagebox
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
                return {
                    'version': available_version,
                    'download_url': update_info.get('html_url', ''),
                    'changelog': update_info.get('body', 'No changelog available'),
                    'release_date': update_info.get('published_at', ''),
                    'download_count': update_info.get('assets', [{}])[0].get('download_count', 0)
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
        """Get the actual download URL from GitHub release"""
        try:
            # For GitHub releases, we need to get the asset download URL
            release_url = update_info.get('download_url', '')
            if 'github.com' in release_url:
                # Extract the release ID and get assets
                release_id = release_url.split('/')[-1]
                assets_url = f"https://api.github.com/repos/stryyk3r/ARKADEManager/releases/{release_id}/assets"
                
                response = requests.get(assets_url, timeout=10)
                if response.status_code == 200:
                    assets = response.json()
                    for asset in assets:
                        if asset['name'].endswith('.zip'):
                            return asset['browser_download_url']
                            
            return update_info.get('download_url', '')
            
        except Exception as e:
            self.log(f"Error getting download URL: {str(e)}")
            return update_info.get('download_url', '')
