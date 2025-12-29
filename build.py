#!/usr/bin/env python3
"""
Automated build script for Arkade Manager EXE
Run this script to automatically build and deploy the EXE
"""

import os
import sys
import subprocess
import shutil
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
    """Check if Python is available"""
    print("[1/6] Checking Python installation...")
    try:
        version = sys.version
        print(f"Python version: {version.split()[0]}")
        return True
    except Exception as e:
        print(f"ERROR: Python check failed: {e}")
        return False

def check_pyinstaller():
    """Check and install PyInstaller if needed"""
    print("\n[2/6] Checking PyInstaller installation...")
    try:
        result = run_command([sys.executable, "-m", "pip", "show", "pyinstaller"], check=False)
        if result.returncode != 0:
            print("PyInstaller not found. Installing...")
            run_command([sys.executable, "-m", "pip", "install", "--quiet", "pyinstaller"])
            print("PyInstaller installed successfully.")
        else:
            print("PyInstaller is already installed.")
        return True
    except Exception as e:
        print(f"ERROR: Failed to check/install PyInstaller: {e}")
        return False

def clean_build():
    """Clean previous build artifacts"""
    print("\n[3/6] Cleaning previous build artifacts...")
    dirs_to_remove = ["build", "dist"]
    for dir_name in dirs_to_remove:
        dir_path = Path(dir_name)
        if dir_path.exists():
            print(f"Removing {dir_name} directory...")
            shutil.rmtree(dir_path)
    print("Cleanup complete.")

def build_exe():
    """Build the EXE using PyInstaller"""
    print("\n[4/6] Building EXE with PyInstaller...")
    spec_file = Path("ArkadeManager.spec")
    if not spec_file.exists():
        print("ERROR: ArkadeManager.spec not found!")
        return False
    
    try:
        run_command([sys.executable, "-m", "PyInstaller", "--clean", str(spec_file)])
        return True
    except subprocess.CalledProcessError as e:
        print(f"ERROR: Build failed with return code {e.returncode}")
        return False

def deploy_exe():
    """Copy the built EXE to the main directory"""
    print("\n[5/6] Deploying EXE to main directory...")
    exe_source = Path("dist") / "ArkadeManager.exe"
    exe_dest = Path("ArkadeManager.exe")
    exe_backup = Path("ArkadeManager.exe.old")
    
    if not exe_source.exists():
        print("ERROR: Built EXE not found in dist folder!")
        return False
    
    # Backup old EXE if it exists
    if exe_dest.exists():
        print("Backing up old EXE...")
        if exe_backup.exists():
            exe_backup.unlink()
        exe_dest.rename(exe_backup)
    
    # Copy new EXE
    print("Copying new EXE...")
    shutil.copy2(exe_source, exe_dest)
    print(f"New EXE deployed: {exe_dest.absolute()}")
    return True

def verify_build():
    """Verify the build was successful"""
    print("\n[6/6] Verifying build...")
    exe_path = Path("ArkadeManager.exe")
    if exe_path.exists():
        size = exe_path.stat().st_size / (1024 * 1024)  # Size in MB
        print(f"✓ EXE exists: {exe_path.absolute()}")
        print(f"✓ EXE size: {size:.2f} MB")
        return True
    else:
        print("✗ EXE not found!")
        return False

def main():
    """Main build process"""
    print("=" * 50)
    print("Arkade Manager EXE Builder")
    print("=" * 50)
    print()
    
    # Change to script directory
    os.chdir(Path(__file__).parent)
    
    steps = [
        ("Python Check", check_python),
        ("PyInstaller Check", check_pyinstaller),
        ("Clean Build", clean_build),
        ("Build EXE", build_exe),
        ("Deploy EXE", deploy_exe),
        ("Verify Build", verify_build),
    ]
    
    for step_name, step_func in steps:
        if not step_func():
            print(f"\n{'=' * 50}")
            print(f"ERROR: {step_name} failed!")
            print(f"{'=' * 50}")
            sys.exit(1)
    
    print("\n" + "=" * 50)
    print("Build and deployment successful!")
    print("=" * 50)
    print()
    print(f"New EXE: {Path('ArkadeManager.exe').absolute()}")
    if Path("ArkadeManager.exe.old").exists():
        print(f"Old EXE backup: {Path('ArkadeManager.exe.old').absolute()}")
    print(f"Build artifacts: {Path('dist/ArkadeManager.exe').absolute()}")
    print()
    print("You can now run ArkadeManager.exe")
    print()

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\nBuild cancelled by user.")
        sys.exit(1)
    except Exception as e:
        print(f"\n\nUnexpected error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

