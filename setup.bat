@echo off
setlocal enabledelayedexpansion

echo ========================================
echo Arkade Manager Setup
echo ========================================
echo.

REM Check if Python is available
python --version >nul 2>&1
if errorlevel 1 (
    echo ERROR: Python is not found in PATH!
    echo.
    echo Please install Python 3.8 or higher from:
    echo https://www.python.org/downloads/
    echo.
    echo Make sure to check "Add Python to PATH" during installation.
    echo.
    pause
    exit /b 1
)

echo [1/4] Checking Python installation...
python --version
echo.

echo [2/4] Checking pip installation...
python -m pip --version >nul 2>&1
if errorlevel 1 (
    echo ERROR: pip is not available!
    echo Please reinstall Python with pip included.
    pause
    exit /b 1
)
echo pip is available.
echo.

echo [3/4] Installing dependencies...
echo Upgrading pip...
python -m pip install --upgrade pip --quiet
if errorlevel 1 (
    echo WARNING: Failed to upgrade pip, continuing anyway...
)

echo Installing dependencies from requirements.txt...
python -m pip install -r requirements.txt
if errorlevel 1 (
    echo.
    echo ERROR: Failed to install dependencies!
    echo Please check the error messages above.
    pause
    exit /b 1
)
echo.

echo [4/4] Verifying installation...
python -c "import requests; import schedule; from PIL import Image; print('All dependencies verified successfully!')" 2>nul
if errorlevel 1 (
    echo WARNING: Some dependencies may not be installed correctly.
    echo Try running: pip install -r requirements.txt
) else (
    echo All required modules are available.
)
echo.

echo ========================================
echo Setup completed successfully!
echo ========================================
echo.
echo You can now run Arkade Manager:
echo   - If EXE exists: Double-click ArkadeManager.exe
echo   - From source: python main.py
echo   - Build EXE: python build.py
echo.
pause

