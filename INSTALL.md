# Installation Guide for Arkade Manager

## Fresh Installation (New Machine)

### Option 1: Using the Setup Script (Recommended)

1. **Download the latest release from GitHub**
   - Go to: https://github.com/stryyk3r/ARKADEManager/releases
   - Download the latest release ZIP file
   - Extract it to your desired location (e.g., `C:\ArkadeManager`)

2. **Run the setup script**
   - **Windows**: Double-click `setup.bat` or run it from command prompt
   - **Linux/Mac**: Run `chmod +x setup.sh && ./setup.sh`
   - **Cross-platform**: Run `python setup.py`

   The setup script will:
   - Check Python installation
   - Install all required dependencies
   - Verify the installation
   - Optionally build the EXE

3. **Run the application**
   - **If EXE exists**: Double-click `ArkadeManager.exe`
   - **If no EXE**: Run `python main.py`

### Option 2: Manual Installation

1. **Download the latest release from GitHub**
   - Go to: https://github.com/stryyk3r/ARKADEManager/releases
   - Download the latest release ZIP file
   - Extract it to your desired location

2. **Install Python** (if not already installed)
   - Download Python 3.8+ from https://www.python.org/downloads/
   - Make sure to check "Add Python to PATH" during installation

3. **Install dependencies**
   ```batch
   pip install -r requirements.txt
   ```

4. **Run the application**
   - **Option A**: Build the EXE first (recommended)
     ```batch
     python build.py
     ```
     Then run `ArkadeManager.exe`
   
   - **Option B**: Run directly from source
     ```batch
     python main.py
     ```

## Updating an Existing Installation

If you already have Arkade Manager installed:

1. **Open Arkade Manager**
2. **Click "🔄 Check for Updates"** button
3. **Confirm the update** when prompted
4. The application will automatically:
   - Download the update
   - Install the files
   - Rebuild the EXE (if applicable)
   - Restart with the new version

**No manual steps required!**

## Troubleshooting

### "ModuleNotFoundError: No module named 'schedule'"

This means dependencies aren't installed. Run:
```batch
pip install -r requirements.txt
```

### "Python was not found"

Python is not installed or not in your PATH:
1. Install Python from https://www.python.org/downloads/
2. Make sure to check "Add Python to PATH" during installation
3. Restart your command prompt/terminal

### "Permission denied" errors

On Windows, try running as Administrator:
- Right-click the command prompt
- Select "Run as administrator"
- Then run the installation commands

### EXE won't start

1. Make sure all dependencies are installed: `pip install -r requirements.txt`
2. Rebuild the EXE: `python build.py`
3. Check if Python is properly installed and in PATH

## Required Dependencies

The application requires:
- Python 3.8 or higher
- `requests` - For update checking and API calls
- `schedule` - For backup scheduling

All dependencies are listed in `requirements.txt` and will be installed automatically by the setup script.

## Data Storage

Your data (backup jobs, logs, configuration) is stored in:
- **Windows**: `%USERPROFILE%\ArkadeManager\`
- **Linux/Mac**: `~/ArkadeManager/`

This location is separate from the application files, so your data persists across updates and reinstalls.

