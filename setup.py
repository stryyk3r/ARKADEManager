#!/usr/bin/env python3
"""
Setup script for Arkade Manager
Installs dependencies and verifies installation
"""

import os
import sys
import subprocess
from pathlib import Path

def run_command(cmd, check=True):
    """Run a command and return the result"""
    print(f"Running: {' '.join(cmd)}")
    result = subprocess.run(cmd, check=check, capture_output=True, text=True)
    if result.stdout:
        print(result.stdout)
    if result.stderr and result.returncode != 0:
        print(result.stderr, file=sys.stderr)
    return result

def check_python():
    """Check if Python is available and correct version"""
    print("=" * 50)
    print("Arkade Manager Setup")
    print("=" * 50)
    print()
    
    print("[1/4] Checking Python installation...")
    try:
        version = sys.version_info
        print(f"Python version: {version.major}.{version.minor}.{version.micro}")
        
        if version.major < 3 or (version.major == 3 and version.minor < 8):
            print("ERROR: Python 3.8 or higher is required!")
            print(f"Current version: {version.major}.{version.minor}.{version.micro}")
            return False
        
        print("✓ Python version is compatible")
        return True
    except Exception as e:
        print(f"ERROR: Python check failed: {e}")
        return False

def check_pip():
    """Check if pip is available"""
    print("\n[2/4] Checking pip installation...")
    try:
        result = run_command([sys.executable, "-m", "pip", "--version"], check=False)
        if result.returncode == 0:
            print("✓ pip is available")
            return True
        else:
            print("ERROR: pip is not available!")
            print("Please install pip or reinstall Python with pip included.")
            return False
    except Exception as e:
        print(f"ERROR: pip check failed: {e}")
        return False

def install_dependencies():
    """Install required dependencies"""
    print("\n[3/4] Installing dependencies...")
    
    requirements_file = Path("requirements.txt")
    if not requirements_file.exists():
        print("ERROR: requirements.txt not found!")
        return False
    
    try:
        # Upgrade pip first
        print("Upgrading pip...")
        run_command([sys.executable, "-m", "pip", "install", "--upgrade", "pip"], check=False)
        
        # Install dependencies
        print("Installing dependencies from requirements.txt...")
        run_command([sys.executable, "-m", "pip", "install", "-r", "requirements.txt"], check=True)
        
        print("✓ Dependencies installed successfully")
        return True
    except subprocess.CalledProcessError as e:
        print(f"ERROR: Failed to install dependencies: {e}")
        return False
    except Exception as e:
        print(f"ERROR: Unexpected error: {e}")
        return False

def verify_installation():
    """Verify that all required modules can be imported"""
    print("\n[4/4] Verifying installation...")
    
    required_modules = ['requests', 'schedule']
    missing_modules = []
    
    for module in required_modules:
        try:
            __import__(module)
            print(f"✓ {module} is available")
        except ImportError:
            print(f"✗ {module} is missing")
            missing_modules.append(module)
    
    if missing_modules:
        print(f"\nERROR: Missing modules: {', '.join(missing_modules)}")
        print("Try running: pip install -r requirements.txt")
        return False
    
    print("\n✓ All required modules are available")
    return True

def main():
    """Main setup process"""
    # Change to script directory
    os.chdir(Path(__file__).parent)
    
    steps = [
        ("Python Check", check_python),
        ("Pip Check", check_pip),
        ("Install Dependencies", install_dependencies),
        ("Verify Installation", verify_installation),
    ]
    
    for step_name, step_func in steps:
        if not step_func():
            print(f"\n{'=' * 50}")
            print(f"ERROR: {step_name} failed!")
            print(f"{'=' * 50}")
            print("\nPlease fix the error above and try again.")
            sys.exit(1)
    
    print("\n" + "=" * 50)
    print("Setup completed successfully!")
    print("=" * 50)
    print()
    print("You can now run Arkade Manager:")
    print("  - If EXE exists: Double-click ArkadeManager.exe")
    print("  - From source: python main.py")
    print("  - Build EXE: python build.py")
    print()

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\nSetup cancelled by user.")
        sys.exit(1)
    except Exception as e:
        print(f"\n\nUnexpected error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

