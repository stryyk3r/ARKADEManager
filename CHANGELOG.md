# Changelog

All notable changes to ARKADE Manager are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [2.3.2] - 2026-07-02

### Added

- **Data Lookup tab** — search for player profiles by EOSID or tribe files by TribeID across all configured ARK backup jobs, with bulk delete support and path safety checks.

---

## [2.3.0] - 2026-06-21

### Changed

- Redesigned UI with sidebar navigation, summary cards, and grouped backup jobs table
- Admin Portal–style buttons, dropdowns, and navigation
- Light theme contrast and readability fixes

### Added

- Release Notes link to GitHub releases
- ARKADE primary logo in sidebar

---

## [2.2.2] - 2026-03-26

### Added

- **Monthly cluster (per job)** — each backup job explicitly selects which FOTM subfolder (`ASA Legacy`, `ASE Legacy`, `ASA Omega`, `Minecraft`, `Palworld`) receives the first two monthly copies.
- **`get_monthly_status`** — Monthly Status reports per-job completion for the current month (copies in the month folder), not a file preview list.

### Changed

- **Monthly backup copy** — ownership uses strict `{job_name}_YYYYMMDD_HHMMSS` parsing and longest job-name match when several jobs could match the same filename; no longer uses file modified time (avoids wrong job / missed new jobs).
- **Run Monthly Backup** — copies use the owning job’s cluster folder correctly.

### Fixed

- **Run Now** could only run once per job in a row; removed the queue pop dedup that dropped the second manual run.

---

## [2.2.0] - 2025-02-25

### Added

- **Minecraft backup support**
  - New backup job type: choose "Minecraft" when creating a job to back up a Minecraft server root.
  - Optional RCON integration: configure host, port, and password for `save-off` / `save-all flush` / `save-on` to get consistent world state during backup.
  - Staging copy when using RCON: server files are copied to a temp directory after flushing, then compressed, so the live server is not held during 7-Zip.
  - 7-Zip support for Minecraft backups when available (better compression); falls back to built-in ZIP.
  - Progress bar in the UI for Minecraft backups (0–100%).
  - Minecraft-specific retention: keep all backups from the last 48 hours, plus the first backup of each day for the last 30 days.
- **Logs tab**
  - "Open Logs Folder" button to open the logs directory in the system file manager.
- **Backups tab**
  - "Open Backup Location" (open destination folder in Explorer) for each job.
- **Scheduler**
  - Only one backup runs at a time (ARK or Minecraft); others wait in a queue.
  - Cooldown after each run to avoid re-queuing the same job too soon (2 minutes after success, 5 minutes after failure).
  - `backup_failed` event so the UI can show when a backup fails.
- **Logging**
  - One log file per app launch with timestamp in the filename (e.g. `arkade_manager_20250225_143022.log`).
  - Timestamps and RFC3339 format in log lines.

### Changed

- **ARK backups**
  - More robust finalization: ZIP is synced and closed before rename; rename uses retries and a copy+remove fallback on Windows when the backup file is locked (e.g. by antivirus or OneDrive).
- **Monthly archive**
  - Now includes backups from ARK jobs only (Minecraft backup destinations are excluded from the monthly archive).
- **Error handling**
  - Backup failures store the full error chain (`{:#}`) and emit `backup_failed` for the UI.
  - Zero-byte backup files are treated as failures and reported to the user.

### Fixed

- Safer handling of locked backup files on Windows (retry then copy+remove fallback).
- Scheduler correctly detects when a backup job has finished before starting the next queued job.

---

## [2.1.x] - Plugin Toggle

- Plugin Toggle tab and related functionality.

## [2.0.0] - Rust migration

- Application migrated to Tauri v2 (Rust backend + web frontend).
- ARK ASA backup jobs, scheduling, monthly archive, logs.

[2.3.2]: https://github.com/stryyk3r/ARKADEManager/releases/tag/v2.3.2
[2.3.0]: https://github.com/stryyk3r/ARKADEManager/releases/tag/v2.3.0
[2.2.2]: https://github.com/stryyk3r/ARKADEManager/releases/tag/v2.2.2
[2.2.0]: https://github.com/stryyk3r/ARKADEManager/releases/tag/v2.2.0
