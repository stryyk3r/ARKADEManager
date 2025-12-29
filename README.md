# ARKADE Manager

Arkade Manager - A comprehensive management tool for ARK: Survival Ascended and Palworld servers.

## Building the EXE

### Automated Build (Recommended)

Simply run one of these commands:

**Windows:**
```batch
build_exe.bat
```

**Linux/Mac:**
```bash
chmod +x build_exe.sh
./build_exe.sh
```

**Cross-platform (Python):**
```bash
python build.py
```

The build script will automatically:
- Check Python installation
- Install PyInstaller if needed
- Clean previous builds
- Build the EXE
- Deploy it to the main directory
- Backup the old EXE

### Manual Build

If you prefer to build manually:

```bash
pip install pyinstaller
python -m PyInstaller ArkadeManager.spec
```

The EXE will be in the `dist` folder.

## Features

- Automated backup management
- Plugin management
- Server configuration editing
- Update checker with automatic updates
- Theme support (Dark/Light)

## Requirements

- Python 3.8+
- See `requirements.txt` for dependencies

## Installation

### Quick Start (Recommended - Pre-built EXE)

1. **Download the latest release** from [GitHub Releases](https://github.com/stryyk3r/ARKADEManager/releases)
2. **Download** `ArkadeManager-vX.X.X.zip` (pre-built EXE)
3. **Extract** to your desired location
4. **Run** `ArkadeManager.exe` - No Python or dependencies needed!

### Alternative: Install from Source

If you prefer to run from source or build your own EXE:

1. **Download the latest release** ZIP file
2. **Extract** to your desired location
3. **Run the setup script**:
   - **Windows**: Double-click `setup.bat`
   - **Linux/Mac**: Run `chmod +x setup.sh && ./setup.sh`
   - **Cross-platform**: Run `python setup.py`
4. **Run the application**:
   - Build EXE: `python build.py` then run `ArkadeManager.exe`
   - From source: `python main.py`

**See [INSTALL.md](INSTALL.md) for detailed installation instructions.**

Your data (backup jobs, logs, config) is stored in `%USERPROFILE%\ArkadeManager\` and persists across updates. 
