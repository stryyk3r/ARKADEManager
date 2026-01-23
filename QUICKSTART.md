# Quick Start Guide

## First Time Setup

1. **Install Prerequisites**:
   - Install Rust: https://rustup.rs/
   - Install Node.js (v18+): https://nodejs.org/
   - Install Tauri CLI: `npm install -g @tauri-apps/cli@latest`

2. **Install Dependencies**:
   ```bash
   npm install
   ```

3. **Create Icon File** (required for building):
   - Create or obtain a Windows `.ico` file
   - Place it at: `src-tauri/icons/icon.ico`
   - Recommended: 256x256 with multiple embedded sizes

4. **Run in Development**:
   ```bash
   npm run tauri dev
   ```

## Creating Your First Backup Job

1. Open the **Backups** tab
2. Click **Browse** next to "Server Root Directory"
   - Select your ARK ASA server root (e.g., `C:\arkservers\asaservers\omega-forglar`)
3. Click **Browse** next to "Destination Directory"
   - Select where backups should be saved
4. Select a **Map** from the dropdown
5. Check the **Include** options you want:
   - Player/Tribe saves
   - Map save
   - Server INI files
   - Plugin config files
6. Set **Interval** (e.g., 6 hours)
7. Set **Retention** (e.g., 7 days)
8. Check **Enabled** to activate the job
9. Enter a **Job Name** and click **Add Job**

The job will appear in the table and will run automatically according to its schedule.

## Manual Backup

1. Select a job from the table
2. Click **Run Now** to execute immediately

## Monthly Archives

- Click **Monthly Status** to preview which backups would be archived
- Click **Run Monthly Backup** to archive the oldest 2 backups from the current month
- Archives are stored in: `C:\arkade\Arkade Shared Global\FOTM Backups\{YYYY-MM}\ASA\`

## Troubleshooting

**App won't start:**
- Check that Rust and Node.js are properly installed
- Run `npm install` to ensure dependencies are installed
- Check `src-tauri/icons/icon.ico` exists (for builds)

**Backup fails:**
- Verify server root directory exists
- Check that required directories exist (saves, config, plugins)
- Check the Logs tab for detailed error messages
- Ensure destination directory is writable

**Jobs not running:**
- Verify job is **Enabled**
- Check that `next_run_at` is in the past
- Review scheduler status in the Status area
- Check logs for errors

## Data Location

All application data is stored in:
- Windows: `%LOCALAPPDATA%\arkade\manager\`

Files:
- `config.json` - Settings
- `backup_jobs.json` - All jobs
- `logs/arkade_manager.log` - Application logs

