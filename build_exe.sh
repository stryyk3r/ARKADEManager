#!/bin/bash

echo "========================================"
echo "Arkade Manager EXE Builder"
echo "========================================"
echo ""

# Check if Python is available
if ! command -v python3 &> /dev/null; then
    echo "ERROR: Python 3 is not found in PATH!"
    echo "Please install Python 3 or add it to your PATH."
    exit 1
fi

echo "[1/5] Checking Python installation..."
python3 --version
echo ""

echo "[2/5] Checking/Installing PyInstaller..."
if ! python3 -m pip show pyinstaller &> /dev/null; then
    echo "PyInstaller not found. Installing..."
    python3 -m pip install --quiet pyinstaller
    if [ $? -ne 0 ]; then
        echo "ERROR: Failed to install PyInstaller!"
        exit 1
    fi
    echo "PyInstaller installed successfully."
else
    echo "PyInstaller is already installed."
fi
echo ""

echo "[3/5] Cleaning previous build artifacts..."
if [ -d "build" ]; then
    echo "Removing build directory..."
    rm -rf build
fi
if [ -d "dist" ]; then
    echo "Removing dist directory..."
    rm -rf dist
fi
echo "Cleanup complete."
echo ""

echo "[4/5] Building EXE with PyInstaller..."
python3 -m PyInstaller --clean ArkadeManager.spec
if [ $? -ne 0 ]; then
    echo ""
    echo "========================================"
    echo "ERROR: Build failed!"
    echo "Check the output above for details."
    echo "========================================"
    exit 1
fi
echo ""

echo "[5/5] Copying EXE to main directory..."
if [ -f "dist/ArkadeManager.exe" ]; then
    echo "Backing up old EXE..."
    if [ -f "ArkadeManager.exe" ]; then
        if [ -f "ArkadeManager.exe.old" ]; then
            rm -f ArkadeManager.exe.old
        fi
        mv ArkadeManager.exe ArkadeManager.exe.old
    fi
    
    echo "Copying new EXE..."
    cp dist/ArkadeManager.exe ArkadeManager.exe
    if [ $? -ne 0 ]; then
        echo "ERROR: Failed to copy EXE!"
        exit 1
    fi
    
    echo ""
    echo "========================================"
    echo "Build and deployment successful!"
    echo "========================================"
    echo ""
    echo "New EXE: ArkadeManager.exe"
    echo "Old EXE backup: ArkadeManager.exe.old"
    echo "Build artifacts: dist/ArkadeManager.exe"
    echo ""
    echo "You can now run ArkadeManager.exe"
    echo ""
else
    echo ""
    echo "========================================"
    echo "ERROR: EXE not found in dist folder!"
    echo "Build may have failed. Check output above."
    echo "========================================"
    exit 1
fi

echo "Build process completed successfully!"

