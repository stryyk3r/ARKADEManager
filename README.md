# ARKADE Manager

Windows desktop app for scheduled game-server backups and admin tooling. Built with Tauri v2 (Rust backend + Vite frontend).

Supported games: **ARK ASA/ASE**, **Minecraft**, and **Palworld**.

## Features

| Tab | Description |
|-----|-------------|
| **Backups** | Create, edit, and run backup jobs; monthly archive preview/run; job filters and status |
| **Logs** | View application logs; open logs folder |
| **Plugin Manager** | Install plugins from a source folder to multiple ARK servers |
| **Plugin Toggle** | Enable/disable plugin folders (`_OFF` suffix) per server or all servers |
| **Data Lookup** | Find and bulk-delete ARK player/tribe save files by EOS ID or tribe ID |
| **Settings** | Theme, server root paths, ARK map list, monthly archive destination |

### Backup behavior

- **ARK**: ZIP backup with optional pre-backup `SaveWorld` via RCON (read from `GameUserSettings.ini`)
- **Minecraft**: Selective world/config backup; optional RCON save-off/flush/save-on; 7-Zip when available
- **Palworld**: REST API save (`PalWorldSettings.ini`) then selective ZIP of save data

## Development

### Prerequisites

- Rust (latest stable)
- Node.js 18+
- Windows (target platform)

### Setup

```bash
npm install
npm run tauri dev
```

Vite serves the frontend on `http://localhost:1420` with hot reload.

### Build installer

```bash
npm run tauri build
```

Output: `src-tauri/target/release/bundle/` (NSIS/MSI depending on config).

Icon: `src-tauri/icons/icon.ico`

## Data storage

**Location:** `%LOCALAPPDATA%\arkade\manager\`

| File | Purpose |
|------|---------|
| `config.json` | Theme, server roots, monthly destination, ARK maps |
| `backup_jobs.json` | Backup job definitions |
| `logs/arkade_manager_*.log` | Per-launch log files |

## Project structure

```
ARKADEManager/
├── index.html              # App shell (tabs, modals, wizard markup)
├── src/
│   ├── main.js             # Bootstrap entry
│   ├── state.js            # Shared UI state
│   ├── styles/             # CSS split by area (base, layout, tables, …)
│   ├── utils/              # DOM and formatting helpers
│   └── ui/                 # Feature modules (jobs, wizard, plugins, …)
├── scripts/dev/            # Local dev/test PowerShell scripts
├── src-tauri/
│   ├── src/
│   │   ├── main.rs         # App bootstrap and invoke registration
│   │   ├── state.rs        # AppState
│   │   ├── commands/       # Tauri command handlers
│   │   ├── backup/         # ARK, Minecraft, Palworld, monthly, retention
│   │   ├── scheduler.rs    # Job queue and tick loop
│   │   ├── validation.rs   # Job validation and path derivation
│   │   ├── plugins.rs      # Plugin install helpers
│   │   ├── data_lookup.rs  # EOS/tribe file search
│   │   ├── palworld_rest.rs
│   │   ├── server_ini.rs
│   │   └── …
│   └── tauri.conf.json
├── CHANGELOG.md
├── README.md
└── package.json
```

## ARK path derivation

From a server root (e.g. `C:\arkservers\asaservers\my-server`):

- Saves: `{root}\ShooterGame\Saved\SavedArks\{map_folder}`
- Config: `{root}\ShooterGame\Saved\Config\WindowsServer`
- Plugins: `{root}\ShooterGame\Binaries\Win64\ArkApi\Plugins`

## Testing

```bash
cd src-tauri
cargo test
```

## Dev scripts

Located in `scripts/dev/`:

- `setup-test-server.ps1` / `cleanup-test-server.ps1` — local ASA plugin folder for testing
- `test-update.ps1` / `restore-version.ps1` — updater smoke tests
