# Quick Start Guide

## First-time setup

1. Install [Rust](https://rustup.rs/) and [Node.js 18+](https://nodejs.org/)
2. From the repo root:

   ```bash
   npm install
   npm run tauri dev
   ```

3. For release builds, ensure `src-tauri/icons/icon.ico` exists.

## Creating a backup job

Use the **Add Job** wizard on the Backups tab.

### ARK (ASA / ASE)

1. Choose **ARK** as the backup type.
2. Set **Server Root** and **Destination** (Browse buttons).
3. Pick a **Map** and include options (saves, map, INI files, plugin configs).
4. Set interval, retention, monthly cluster, and job name.
5. Save — the scheduler runs enabled jobs automatically.

ARK backups call `SaveWorld` over RCON when `RCONEnabled=True` in `GameUserSettings.ini`.

### Minecraft

1. Choose **Minecraft** as the backup type.
2. Set server root (folder containing `world` and `config`).
3. Optionally set RCON host/port/password for save-off/flush before backup.
4. Backups use 7-Zip when installed; otherwise built-in ZIP.

### Palworld

1. Choose **Palworld** as the backup type.
2. Set server root (SteamCMD install folder).
3. Ensure `PalWorldSettings.ini` has `RESTAPIEnabled=True`, `AdminPassword`, and `RESTAPIPort`.
4. Optional **API Host** override if REST is not on localhost.

Palworld backups trigger a REST save before zipping world and config files.

## Other tabs

- **Logs** — refresh or open the logs folder
- **Plugin Manager** — copy plugins from a source path to selected servers
- **Plugin Toggle** — rename plugin folders to/from `_OFF`
- **Data Lookup** — search ARK save files by EOS ID or tribe ID
- **Settings** — theme, ASA/Minecraft/Palworld server roots, ARK maps, monthly archive path

## Monthly archives

- **Monthly Status** — see which jobs have met their two monthly copies
- **Run Monthly Backup** — copy the first two backups of the current month per job into the configured FOTM folder

Default destination: `C:\arkade\Arkade Shared Global\FOTM Backups\{YYYY-MM-MMMMM}\`

## Troubleshooting

| Issue | Check |
|-------|--------|
| App won't start | `npm install`; Rust/Node on PATH |
| Backup fails | Logs tab; verify paths and game-specific requirements (RCON/REST INI) |
| Jobs not running | Job enabled; scheduler status in sidebar; `next_run_at` in the past |
| Palworld save fails | `RESTAPIEnabled=True` and valid admin password in `PalWorldSettings.ini` |

## Data location

`%LOCALAPPDATA%\arkade\manager\` — `config.json`, `backup_jobs.json`, `logs/`
