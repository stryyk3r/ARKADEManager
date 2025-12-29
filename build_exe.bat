@echo off
setlocal enabledelayedexpansion

echo ========================================
echo Arkade Manager EXE Builder
echo ========================================
echo.

REM Check if Python is available
python --version >nul 2>&1
if errorlevel 1 (
    echo ERROR: Python is not found in PATH!
    echo Please install Python or add it to your PATH.
    pause
    exit /b 1
)

echo [1/5] Checking Python installation...
python --version
echo.

echo [2/5] Checking/Installing PyInstaller...
python -m pip show pyinstaller >nul 2>&1
if errorlevel 1 (
    echo PyInstaller not found. Installing...
    python -m pip install --quiet pyinstaller
    if errorlevel 1 (
        echo ERROR: Failed to install PyInstaller!
        pause
        exit /b 1
    )
    echo PyInstaller installed successfully.
) else (
    echo PyInstaller is already installed.
)
echo.

echo [3/5] Cleaning previous build artifacts...
if exist build (
    echo Removing build directory...
    rmdir /s /q build
)
if exist dist (
    echo Removing dist directory...
    rmdir /s /q dist
)
echo Cleanup complete.
echo.

echo [4/5] Building EXE with PyInstaller...
python -m PyInstaller --clean ArkadeManager.spec
if errorlevel 1 (
    echo.
    echo ========================================
    echo ERROR: Build failed!
    echo Check the output above for details.
    echo ========================================
    pause
    exit /b 1
)
echo.

echo [5/5] Copying EXE to main directory...
if exist dist\ArkadeManager.exe (
    echo Backing up old EXE...
    if exist ArkadeManager.exe (
        if exist ArkadeManager.exe.old del /q ArkadeManager.exe.old
        ren ArkadeManager.exe ArkadeManager.exe.old
    )
    
    echo Copying new EXE...
    copy /y dist\ArkadeManager.exe ArkadeManager.exe >nul
    if errorlevel 1 (
        echo ERROR: Failed to copy EXE!
        pause
        exit /b 1
    )
    
    echo.
    echo ========================================
    echo Build and deployment successful!
    echo ========================================
    echo.
    echo New EXE: ArkadeManager.exe
    echo Old EXE backup: ArkadeManager.exe.old
    echo Build artifacts: dist\ArkadeManager.exe
    echo.
    echo You can now run ArkadeManager.exe
    echo.
) else (
    echo.
    echo ========================================
    echo ERROR: EXE not found in dist folder!
    echo Build may have failed. Check output above.
    echo ========================================
    pause
    exit /b 1
)

echo Build process completed successfully!
timeout /t 3 >nul

