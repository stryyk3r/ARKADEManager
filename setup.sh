#!/bin/bash

echo "========================================"
echo "Arkade Manager Setup"
echo "========================================"
echo ""

# Check if Python is available
if ! command -v python3 &> /dev/null; then
    echo "ERROR: Python 3 is not found in PATH!"
    echo ""
    echo "Please install Python 3.8 or higher from:"
    echo "https://www.python.org/downloads/"
    echo ""
    exit 1
fi

echo "[1/4] Checking Python installation..."
python3 --version
echo ""

echo "[2/4] Checking pip installation..."
if ! python3 -m pip --version &> /dev/null; then
    echo "ERROR: pip is not available!"
    echo "Please reinstall Python with pip included."
    exit 1
fi
echo "pip is available."
echo ""

echo "[3/4] Installing dependencies..."
echo "Upgrading pip..."
python3 -m pip install --upgrade pip --quiet

echo "Installing dependencies from requirements.txt..."
python3 -m pip install -r requirements.txt
if [ $? -ne 0 ]; then
    echo ""
    echo "ERROR: Failed to install dependencies!"
    echo "Please check the error messages above."
    exit 1
fi
echo ""

echo "[4/4] Verifying installation..."
python3 -c "import requests; import schedule; print('All dependencies verified successfully!')" 2>/dev/null
if [ $? -ne 0 ]; then
    echo "WARNING: Some dependencies may not be installed correctly."
    echo "Try running: pip install -r requirements.txt"
else
    echo "All required modules are available."
fi
echo ""

echo "========================================"
echo "Setup completed successfully!"
echo "========================================"
echo ""
echo "You can now run Arkade Manager:"
echo "  - If EXE exists: ./ArkadeManager.exe"
echo "  - From source: python3 main.py"
echo "  - Build EXE: python3 build.py"
echo ""

