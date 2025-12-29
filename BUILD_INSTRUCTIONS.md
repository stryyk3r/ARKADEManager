# Building Arkade Manager EXE

## Prerequisites

1. Python 3.x installed
2. All dependencies installed: `pip install -r requirements.txt`
3. PyInstaller installed: `pip install pyinstaller`

## Quick Build

### Option 1: Use the build script (Windows)
```batch
build_exe.bat
```

### Option 2: Manual build command
```batch
python -m PyInstaller ArkadeManager.spec
```

## Build Process

1. The build script will:
   - Check if PyInstaller is installed (install if needed)
   - Clean previous build artifacts
   - Build the EXE using the spec file
   - Output the EXE to `dist\ArkadeManager.exe`

2. After building:
   - The EXE will be in the `dist` folder
   - Copy `dist\ArkadeManager.exe` to your main directory
   - The EXE includes all dependencies and can run standalone

## Spec File Details

The `ArkadeManager.spec` file includes:
- Main script: `main.py`
- Icon: `arkade_icon.ico`
- Logo: `arkade_logo.png`
- Console: Disabled (windowed application)
- UPX compression: Enabled

## Troubleshooting

- If build fails, check that all dependencies are installed
- Make sure `arkade_icon.ico` and `arkade_logo.png` exist in the root directory
- Check the build output for any missing module errors

