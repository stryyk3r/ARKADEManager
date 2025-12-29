# Manual Installation Guide for Arkade Manager

If the automatic update process fails, follow these steps to manually install a new version:

## Steps:

1. **Close Arkade Manager completely**
   - Make sure the application is fully closed (check Task Manager if needed)

2. **Download the latest version**
   - Go to: https://github.com/stryyk3r/ARKADEManager/releases
   - Download the latest release ZIP file

3. **Backup your data (IMPORTANT)**
   - Your backup jobs are stored in: `%USERPROFILE%\ArkadeManager\backup_jobs.json`
   - This file is automatically preserved, but it's good to have a backup

4. **Extract the ZIP file**
   - Extract the downloaded ZIP to a temporary location (e.g., Desktop)

5. **Copy files to your current installation**
   - Navigate to your current Arkade Manager folder
   - Copy ALL files from the extracted folder to your current folder
   - **IMPORTANT:** Replace all files EXCEPT:
     - `backup_jobs.json` (if it exists in the old location - your data is in %USERPROFILE%\ArkadeManager)
     - Any custom configuration files you've modified

6. **Restart Arkade Manager**
   - Launch the application
   - Verify the version number in the title bar

## Troubleshooting:

- If you get permission errors, try running as Administrator
- If files are locked, make sure Arkade Manager is completely closed
- Your backup jobs are stored in `%USERPROFILE%\ArkadeManager\` and will NOT be lost during updates

## Current Installation Location:

Your current installation is located at:
```
[The folder where you installed Arkade Manager]
```

Your data (backup jobs, logs, config) is stored at:
```
%USERPROFILE%\ArkadeManager\
```

This location is separate from the application files, so your data is safe during updates.

