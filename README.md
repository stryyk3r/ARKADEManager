# ARKADE Manager

ARKADE Manager is a Windows desktop application for managing ARK: Survival Ascended server backups and configurations. Built with Tauri v2 (Rust backend + web frontend).

## Features

### Phase 1 (Current)
- **Backups Tab**: Full backup job management for ARK ASA
  - Create, update, delete backup jobs
  - Automatic scheduled backups
  - Manual backup execution
  - Monthly archive system
  - Job status monitoring
- **Logs Tab**: View application logs
- **Other Tabs**: Stubbed for future implementation

## Development

### Prerequisites

- **Rust** (latest stable version)
- **Node.js** (v18 or later)
- **Windows** (target platform)

### Setup

1. Install dependencies:
```bash
npm install
```

2. Run in development mode:
```bash
npm run tauri dev
```

This will:
- Start the Vite dev server on `http://localhost:1420`
- Compile and run the Tauri application
- Enable hot-reload for frontend changes

### Building

**Note**: Before building, you need to create an icon file:
- Place `icon.ico` in `src-tauri/icons/` directory
- The icon should be a Windows .ico file (recommended: 256x256 or larger with multiple sizes)

To build a Windows installer:

```bash
npm run tauri build
```

The installer will be generated in `src-tauri/target/release/bundle/msi/`

## Data Storage

Application data is stored in the Windows AppData directory:

**Location**: `%LOCALAPPDATA%\arkade\manager\`

**Files**:
- `config.json` - Application configuration (theme, monthly archive destination)
- `backup_jobs.json` - All backup job definitions
- `logs/arkade_manager.log` - Application logs

## Project Structure

```
arkade_manager_rust/
├── src/                    # Frontend source (HTML/JS)
│   └── main.js            # Main frontend logic
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── main.rs       # Tauri entry point, command handlers
│   │   ├── app_data.rs   # Data persistence layer
│   │   ├── backup.rs     # Backup creation and monthly archives
│   │   ├── config.rs     # Configuration model
│   │   ├── job.rs        # Job model and scheduling logic
│   │   ├── map.rs        # ARK ASA map definitions
│   │   ├── scheduler.rs  # Job scheduler and queue
│   │   └── validation.rs # Path derivation and validation
│   ├── Cargo.toml        # Rust dependencies
│   └── tauri.conf.json   # Tauri configuration
├── index.html            # Main HTML file
├── package.json          # Node.js dependencies
└── vite.config.js       # Vite configuration
```

## ARK ASA Directory Structure

The application derives all paths from a single server root directory:

**Server Root**: User-defined (e.g., `C:\arkservers\asaservers\omega-forglar`)

**Derived Paths**:
- **Saves**: `{root_dir}\ShooterGame\Saved\SavedArks\{map_folder_name}`
- **Config**: `{root_dir}\ShooterGame\Saved\Config\WindowsServer`
- **Plugins**: `{root_dir}\ShooterGame\Binaries\Win64\ArkApi\Plugins`

## Backup Jobs

Each backup job stores:
- Server root directory
- Destination directory
- Map selection (12 ARK ASA maps supported)
- Include toggles (saves, map, server files, plugin configs)
- Schedule (interval value + unit: minutes/hours/days)
- Retention days
- Enabled status

Backups are created as timestamped ZIP files with compression level 5.

## Monthly Archives

Monthly archives run on the 1st of each month at 11 PM local time (TODO: implement scheduled trigger). They archive the oldest 2 backups from the current month to a configurable destination (default: `C:\arkade\Arkade Shared Global\FOTM Backups`).

## Testing

Run unit tests:

```bash
cd src-tauri
cargo test
```

Tests cover:
- Path derivation from root_dir + map
- Next run calculation
- Retention selection
- Monthly archive selection
- Dedup logic

## Future Enhancements

- **Palworld Support**: Stubbed for Phase 2
- **New Plugins Tab**: Plugin discovery and installation
- **Game.ini / GameUserSettings.ini Tabs**: INI file editors
- **Tribe Files Tab**: Tribe file management
- **.ini Editor Tab**: Generic INI editor
- **IP Updater Tab**: Server IP management
- **Plugin Manager Tab**: Plugin lifecycle management

## License

[Your License Here]

