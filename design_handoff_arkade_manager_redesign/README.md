# Handoff: ARKADE Manager full UI redesign

## Overview
Redesign of every screen in ARKADE Manager (`stryyk3r/ARKADEManager`, Tauri v2 + Vite, vanilla JS). Goals: remove the "AI-generated" look (gradients, glows, icon-tile KPI cards), make Logs readable, rebuild Plugins / Toggle / Data Lookup / Settings, make Backups game groups collapsible, and replace the hidden inline job form with a side drawer.

Scope:
- Backups (grouped, collapsible jobs table)
- Edit Job (right-side drawer)
- Add Job wizard (modal with step rail)
- Monthly Status (modal)
- Logs
- Plugins: one sidebar item with two tabs, **Install** and **Enable / disable** (merges the old `new-plugins` and `plugin-toggle` tabs)
- Data Lookup
- Settings

Dark theme only for this pass. The existing light-theme toggle stays in Settings, but light tokens are not redesigned.

## About the design files
The `.dc.html` files in `designs/` are **design references built in HTML**. They show the intended look and behavior; they are not production code. Rebuild them in the existing app (`index.html`, `src/styles/*.css`, `src/ui/**/*.js`) using its current patterns: plain DOM rendering, `data-action` buttons, and Tauri `invoke` calls. Keep every existing Tauri command and event (`list_jobs`, `get_status`, `status_update`, `job_updated`, `backup_progress`, `backup_failed`, etc.). Sample data in the mocks is illustrative only.

Open `designs/ARKADE Manager Redesign.dc.html` in a browser to see all screens on one canvas. Each screen is also its own file (1280×800; Settings is 1280×1180). They need `support.js` next to them.

## Fidelity
**High-fidelity.** Colors, type, spacing and copy are final. Match them closely.

---

## Design tokens

Replace the values in `src/styles/base.css` `:root` (dark) with these. Keep the variable names where they already exist.

### Colors
| Token | Hex | Use |
|---|---|---|
| bg-base | `#020F22` | App/main background |
| bg-sidebar | `#010A18` | Sidebar |
| surface-panel | `#051326` | Tables, panels |
| surface-group | `#041021` | Table group-header rows, sub-headers |
| surface-raised | `#06152B` | Drawer, modals |
| surface-footer | `#071A33` | Sticky action bars at panel bottoms |
| surface-input | `#030C1C` | Inputs, segmented controls, selects |
| surface-btn | `#061730` | Secondary button fill |
| seg-active | `#0C2340` | Active segment in segmented controls |
| menu | `#0A1D38` | Dropdown menus |
| border-hair | `rgba(255,255,255,.05)` | Row dividers |
| border-subtle | `rgba(255,255,255,.07)` | Section dividers |
| border-default | `rgba(255,255,255,.08)` | Panel borders, table header bottom |
| border-control | `rgba(255,255,255,.10)` | Input borders |
| border-btn | `rgba(255,255,255,.12)` / `.14` | Secondary button borders |
| text-primary | `#F2F0F8` | Primary text |
| text-strong-2 | `#E6E4F2` | Table values |
| text-body | `#D9D7EA` | Button labels, body on panels |
| text-secondary | `#A9A6C4` | Descriptions, secondary cells |
| text-muted | `#8A85AB` | Labels, column headers, relative times |
| text-faint | `#6F6B92` | Section labels in sidebar, unchecked checkbox borders |
| cyan | `#1FE3F5` | Selection, focus, running state, active nav bar, checkboxes, switches |
| cyan-light | `#8FF3FB` | Link hover |
| magenta | `#DD3DEE` | **Primary action only** (one per screen). Text on it: `#16031C`, weight 800 |
| magenta-text | `#F5C8FA` | "Pending" labels |
| green | `#34d399` | OK / scheduled / found |
| yellow | `#fbbf24` | Warning / paused-partial / outdated |
| red | `#f87171` | Error / destructive |
| red-text | `#FCA5A5` | Error log text, destructive button label |
| game-asa | `#1FE3F5` | ARK: Survival Ascended |
| game-ase | `#fbbf24` | ARK: Survival Evolved |
| game-mc | `#34d399` | Minecraft |
| game-pal | `#7AA7FF` | Palworld |

Tints: game and state chips use the hue at 14% alpha (`rgba(31,227,245,.14)` and so on). Selected rows use `rgba(31,227,245,.04–.07)`. Pending rows use `rgba(221,61,238,.06)`. Focus ring: `border: 1px solid rgba(31,227,245,.55)` plus `box-shadow: 0 0 0 3px rgba(31,227,245,.1)`.

**Not allowed:** gradients, glows, pulsing dots, coloured drop shadows. Shadows are used only on overlays: menus `0 16px 40px rgba(0,0,0,.5)`, modals `0 30px 80px rgba(0,0,0,.55)`, drawer `-24px 0 60px rgba(0,0,0,.45)`. Overlay scrim: `rgba(1,6,15,.62–.66)`.

### Typography
- UI: **Manrope** 400/500/600/700/800 (replaces Inter in `base.css`).
- Mono: **JetBrains Mono** 400/500/600. Use it for file paths, IDs (EOS ID, Tribe ID), file names, versions, log lines and the version string.
- Use `font-variant-numeric: tabular-nums` for times, sizes and counts.

| Role | Size / weight |
|---|---|
| Page title (h1) | 21px / 800, letter-spacing -.01em |
| Page description | 13px / 400, text-secondary, 4px below title |
| Section title (h2) | 15px / 700 |
| Drawer/modal title | 18px / 800 |
| Body / table cell | 13–13.5px / 500–600 |
| Column header | 11px / 700, uppercase, letter-spacing .06em, text-muted |
| Field label | 12px / 700, text-secondary, 6px above input |
| Small meta | 11.5–12.5px |
| Mono paths | 11.5–12.5px |
| Log lines | JetBrains Mono 12.5px, line-height 1.55 |

### Spacing and radius
- Main content padding: `26px 32px`. Sidebar is 232px wide with `18px 12px 14px` padding.
- Gap scale: 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 40.
- Radius: 3px checkboxes, 4px segment pills and tags, 5–6px inputs and buttons, 8px panels, 10px modals.
- Control heights: buttons and inputs 34px (32px in Settings and dense panels, 30px small); the hero search is 42px. Table rows are 46px, group rows 42px, header rows 32–36px. Sticky footer bars are 58px.

### Shared components
- **Primary button:** bg `#DD3DEE`, no border, text `#16031C` 13px/800, height 34, padding 0 15px, radius 6.
- **Secondary button:** bg `#061730`, border `1px rgba(255,255,255,.12)`, text `#D9D7EA` 13px/600.
- **Ghost button:** transparent, text `#A9A6C4` 13px/600.
- **Destructive:** text-only `#f87171` 700 in footers; filled variant in confirms uses bg `#f87171` with text `#2A0606`; outline variant uses border `rgba(248,113,113,.6)`, bg `rgba(248,113,113,.12)`, text `#FCA5A5`.
- **Segmented control:** container bg `#030C1C`, border `.10`, padding 3px, gap 2px, radius 6. Active segment bg `#0C2340`, text primary, radius 4. Inactive text `#A9A6C4`, 12.5px/600. Segments can carry a count in text-muted.
- **Checkbox:** 15px, radius 3. Off: border `1.5px #6F6B92`. On: bg and border `#1FE3F5` with a `#020F22` check (stroke 4). Indeterminate: a 7×2 bar `#A9A6C4`.
- **Switch:** 30×18 (34×20 in the drawer). On: track `#1FE3F5`, knob `#020F22`. Off: track `#1A2C47`, knob `#8A85AB`.
- **Tabs (Plugins):** 13.5px/700, gap 24, active tab has a 2px `#1FE3F5` underline, row has bottom border `.08`.
- **Sticky action bar:** pinned to the bottom of a panel, 58px tall, bg `#071A33`, top border `.08`. Holds a status dot, a bold summary, muted detail, a spacer, then buttons.

---

## Screens

### Sidebar (`ArkSidebar.dc.html`), used on every screen
- 232px, bg `#010A18`, right border `rgba(255,255,255,.06)`.
- Brand: `Icon_ArkadeLogo.png` at 30px, "ARKADE Manager" 13px/800, version in mono 10.5px muted.
- Section labels "Workspace" and "System": 10.5px/700 uppercase, letter-spacing .1em, `#6F6B92`.
- Nav items, 36px tall, radius 6, 13.5px/600, 16px stroke icons:
  - Workspace: Backups, Logs, Plugins, Data Lookup.
  - System: Settings.
- Active item: bg `rgba(31,227,245,.08)`, text `#F2F0F8`, and a 2px cyan bar on the sidebar's left edge (inset 9px top and bottom). Inactive text is `#A9A6C4`.
- **Nav change:** "Plugins" and "Toggle" become one "Plugins" item.
- Scheduler block: pinned to the bottom, top hairline. It shows a "Scheduler" label with a green dot and "Running" (muted "Idle" when no job runs), then rows for Current, Queue and Last tick. Values are wired to `get_status` as today.

### 1a. Backups (`Screen - Backups.dc.html`)
1. **Header.** "Backup Jobs" with "9 jobs" (13px/600 muted) inline. Description: "Scheduled saves across your game servers." Actions on the right: an icon-only Refresh button (34×34), "Run monthly" (secondary), "+ Add job" (primary).
2. **Status strip** (replaces the four KPI cards). One row with top and bottom hairlines, and cells separated by 1px dividers. Each cell has a label (11.5px/600 muted) over a value (15px/700). Cells:
   - Running: job name in cyan plus percent.
   - Queue.
   - Next save: job name plus "in 12 min".
   - Success, 24h.
   - Needs attention: red "1 failed" and yellow "· 1 warning".
3. **Toolbar.**
   - Search input, 250px, placeholder "Search jobs, maps, paths".
   - Game segmented control (All / ASA / ASE / Minecraft / Palworld). It replaces the type `<select>`; ASA and ASE now filter separately.
   - Status select ("Any status").
   - On the right: a ghost "Collapse all / Expand all" button and a secondary "Monthly status" button.
4. **Table.** Panel `#051326`, radius 8 (top corners only; the table runs to the bottom of the window).
   - Grid columns: `minmax(0,1fr) 96px 176px 176px 92px 36px`, gap 16, padding 0 16. Headers: Job, Interval, Next save, Last save, Size (right-aligned), and an empty menu column.
   - **Group header row:** 42px, bg `#041021`, clickable.
     - Contents: chevron (down when open, right when collapsed), a 20px game chip (letter on a 14% tint, radius 4), the game name 13.5px/700, and meta in muted text, e.g. "3 jobs · ASA Legacy".
     - The group's total size sits in the Size column, so it lines up with the job sizes.
     - Group order: ASA, ASE, Minecraft, Palworld (same as `JOB_GROUP_ORDER`).
   - **Job row:** 46px; the name is indented 34px to sit under the group name.
     - Status dot (7px) + name (13.5px/600) + inline sub-status (12px/600, coloured, ellipsis).
     - Interval reads "30 min".
     - Next save: time (`#E6E4F2`) + relative time (muted), e.g. "8:31 PM in 12 min".
     - Last save: time + "18 min ago".
     - Size: right-aligned, 600.
     - Menu: a 28px ⋯ button.
   - **Row states** (extends `getJobRowState`):

     | State | Dot | Sub-status | Next save | Last save | Row |
     |---|---|---|---|---|---|
     | Running | cyan | "Saving", cyan | progress bar (4px track at `rgba(31,227,245,.15)`, cyan fill) + "64%" cyan/700, from `backup_progress` | normal | bg `rgba(31,227,245,.045)` |
     | Error | red | first line of `last_error`, red (e.g. "RCON connection refused") | "retry in 21 min" | "Failed" red/700 + time | normal |
     | Warning | yellow | "Completed with warnings", yellow | normal | normal | normal |
     | Paused (disabled) | `#6F6B92` | "Paused", muted | "—" | normal | name in `#A9A6C4` |
     | Scheduled | green | none | normal | normal | normal |

     The separate progress card above the table is removed; progress shows in the running row instead.
   - **Row menu:** 200px dropdown, bg `#0A1D38`, border `.12`, radius 8, padding 5. Items are 13px/600 with 8px 10px padding: Run now, Open backup folder, Edit job…, Pause schedule, a divider, then "Delete job" in red. Hover: `rgba(31,227,245,.08)`. Keep the existing viewport-aware positioning (`positionJobMenuDropdown`).
   - Empty state: keep `#backupEmptyAll` with the new button styles.

### 1b. Edit Job drawer (`Screen - Edit Job.dc.html`)
Replaces `#jobFormContainer`. "Edit job…" in the row menu opens it.
- A scrim covers the app. The drawer is 460px wide, anchored right and full height: bg `#06152B`, left border `.10`.
- Suggested animation: slide in from the right with `transform: translateX(100%) → 0`, 200ms ease-out; scrim fades in over 150ms. Esc or a scrim click closes it.
- **Header** (padding 20/22):
  - Game chip and game name (12px/700 in the game colour).
  - Job name, 18px/800.
  - Meta line: "Last save 8:01 PM · 156.2 MB · next in 12 min".
  - Close X on the right.
- **Body** (padding 0 22px; sections separated by hairlines, 16px vertical padding):
  1. Job name and Map side by side; Map only for ARK.
  2. "Every" (number + unit select) and "Keep for" (retention in days) side by side.
  3. Monthly cluster select.
  4. "Include" checkboxes in a 2×2 grid (ARK only): Map save, Player and tribe saves, Server INI files, Plugin configs.
  5. Server root and Destination: mono path inputs with a Browse button each. Under the server root, show the derived saves path in mono 11px muted, e.g. "saves → …\ShooterGame\Saved\SavedArks\TheCenter_WP".
  6. "Schedule enabled" switch with the hint "RCON SaveWorld runs first when enabled in GameUserSettings.ini".
- Minecraft jobs show RCON host, port and password instead of Map and Include. Palworld jobs show the optional API host. Reuse the field sets from the current form.
- **Footer** (bg `#071A33`): "Delete job" text in red on the left; Cancel (secondary) and "Save changes" (primary) on the right.

### 1c. Add Job wizard (`Screen - Add Job.dc.html`)
Same five steps and logic as `backup-wizard.js`. New layout:
- Centred modal, 780×520, radius 10.
- **Left step rail**, 236px, bg `#041021`:
  - Title "New backup job".
  - Vertical stepper: Game, Server root, Destination, Map, Schedule.
  - Each step has a 22px circle and a 1.5px connector line.
    - Done: cyan fill with ✓; connector at `rgba(31,227,245,.5)`.
    - Current: cyan outline and cyan number.
    - Upcoming: `#3A3860` outline and muted number.
  - Under each completed step, show the value chosen (mono for paths, ellipsis). This replaces the old dot indicator.
  - For Minecraft and Palworld, replace the Map step with that game's config step, as the current wizard does.
- **Right pane:**
  - "Step 3 of 5" (12px/700 muted), then the step question as a title, e.g. "Where should backups go?".
  - Step 3 content: mono destination input with Browse.
  - Under the input, a check line: "Writable" in green, free space, and a same-drive note.
  - A preview box: "Files will be named", then `LNA_Center_20261027_203148.zip` in mono with the timestamp in cyan.
  - A "Recently used" list of destinations.
  - Footer: Cancel (ghost) on the left; Back (secondary) and "Next: Map" (primary, names the next step) on the right.

### 1d. Monthly Status (`Screen - Monthly Status.dc.html`)
- 640px modal. Title "Monthly archive · October 2026". Subline: "First 2 backups per job copied to D:\Backups\FOTM" (path in mono).
- Summary row: counts for complete, partial (yellow) and missing (red).
- Grouped by cluster folder (ASA Legacy, ASE Legacy, ASA Omega, Minecraft, Palworld). Group labels are mono 11.5px muted.
- Job row, 34px, grid `1fr 44px 190px`:
  - Name.
  - Two 14×6 pips: green when that copy exists, yellow for a partial second copy, `#1A2C47` when empty.
  - Copy dates, or a reason ("No copies, last run failed") in yellow or red.
- Data comes from `get_monthly_status`.
- Footer: "Close" and "Run monthly" (primary).

### 1e. Logs (`Screen - Logs.dc.html`)
- **Header.** Title "Logs", description "One log file per app launch."
  - On the right: a log-file select (mono, current file tagged "current" in green) and an "Open folder" button.
- **Toolbar.**
  - Level segmented control with counts: All / Info / Warn (yellow) / Error (red).
  - "All sources" select: scheduler, backup, rcon, palworld, monthly, updater.
  - Filter input.
  - Spacer, then a **Follow** switch (auto-scroll to the end) and a ghost "Clear view" button.
- **Viewer** (replaces the `<pre>`):
  - Panel bg `#030B19`. Column header row on `#051326`: Time, Level, Source, Message.
  - Grid `104px 58px 92px 1fr`, gap 14. Everything is mono 12.5px.
  - Time: muted `HH:MM:SS.mmm`.
  - Level text colours: INFO `#7FB8C4`, WARN `#fbbf24`, ERROR `#f87171`, DEBUG `#6F6B92`.
  - Message colours: INFO `#E6E4F2`, WARN `#FDE68A`, ERROR `#FCA5A5`, DEBUG muted.
  - WARN rows: 2px left stripe `#fbbf24`, bg `rgba(251,191,36,.05)`. ERROR rows: stripe `#f87171`, bg `rgba(248,113,113,.07)`.
  - Error chains: split `caused by:` segments onto their own lines (`white-space: pre-wrap`).
- **Footer bar** (32px): a cyan dot with "Following · refreshes every 5s" on the left; the logs folder path in mono on the right.
- Parsing: split each line of the existing RFC3339 log format into time, level, target and message. If a line doesn't parse, show it raw in the Message column.

### 1f. Plugins: Install tab (`Screen - Plugins Install A.dc.html`)
- Title "Plugins". Description: "ArkApi plugins for servers under C:\arkservers\asaservers" (root in mono). Tabs below: Install | Enable / disable.
- Three-column grid `1fr 1fr 300px`, gap 16. Each column is a panel.
  1. **Source plugins.**
     - Header: "1  Source plugins" (the step number in muted text) and "3 of 7 selected" on the right.
     - Mono path field with Browse.
     - "Select all" row with an indeterminate checkbox.
     - Rows 40px: checkbox, plugin name 13px/600, version mono on the right. Selected rows get a cyan tint.
  2. **Destination servers.**
     - Header with a selected count.
     - Grouped by cluster. Group sub-header is 32px on `#041021`, uppercase, with a cyan "Select all" on the right.
     - Rows 44px: checkbox, server name, muted meta ("9 plugins installed" or "Server offline").
     - For selected servers, show a right-aligned impact summary: "2 updates" in yellow, or "3 new" / "2 new · 1 update" in green.
  3. **Review.**
     - Big total, e.g. "12 installs" at 22px/800, with "3 plugins on 4 servers" below.
     - Legend counts: 7 new (green dot), 4 updates (yellow), 1 same (muted).
     - A scrollable list of every install: plugin bold, "→ server" muted, and a mono status on the right ("2.0.4 → 2.1.0" yellow, "new" green, "same" muted).
     - Bottom: the checkbox "Keep each server's existing config.json" (on by default), then a full-width primary "Install 12" button.
- The existing confirm and results modals (`pluginConfirmModal`, `pluginResultsModal`) stay, restyled with the modal tokens. The Review column means the confirm step can be skipped if you prefer.

### 1h. Plugins: Enable / disable tab (`Screen - Plugins Toggle A.dc.html`)
- Description: "Disabled plugin folders carry an _OFF suffix. Restart the server to apply."
- Grid `260px 1fr`, gap 16.
- **Server list** (replaces the `<select>`):
  - "Find server" search.
  - Rows 52px: server name, cluster underneath in muted text, and an "on / installed" count on the right (e.g. "7 / 9").
  - Selected row: cyan tint, 2px cyan left bar, primary text.
- **Folder panel:**
  - Header: server name 15px/700, the plugins path in mono muted below it, and a segmented filter "All 9 / On 7 / Off 2".
  - Rows 46px, grid `44px 1fr 220px 90px`: switch, plugin display name (`_OFF` stripped), the real folder name in mono muted, and a pending label.
  - Off rows show the name in `#A9A6C4`.
- **Staged changes.** Flipping a switch doesn't rename immediately.
  - Changed rows get bg `rgba(221,61,238,.06)` and a "turning on" / "turning off" label in `#F5C8FA`.
  - The sticky action bar shows a magenta dot, "2 pending changes" and the summary "KillFeed off, VoteRewards on".
  - Buttons: Discard (ghost), "Apply to all 6 servers" (secondary), "Apply to lna-center" (primary). These map to the existing `togglePluginsForAllServers` and `togglePluginsForCurrentServer`.
- State: `selectedServer`, `folders[]`, and `pending: Map<folderName, targetOn>`.

### 1j. Data Lookup (`Screen - Data Lookup A.dc.html`)
- Description: "Find and remove a player's profile or a tribe's file across every ASA server."
- **Search panel:**
  - One row: a segmented control "Player · EOS ID" / "Tribe · Tribe ID" (replaces the radios), a 42px mono input (14px, focus ring), and a "Search" primary button.
  - Live validation inside the input on the right: "32 / 32 hex" in green when valid, red when the format is wrong. EOS ID must be 32 hex characters; Tribe ID must be numeric. Disable Search until the value is valid.
  - Under the row: "Searched 6 servers under C:\arkservers\asaservers in 0.8s".
- **Results header:** "6 files found" (15px/700) with "on 5 servers · 1.9 MB" muted.
- **Results table:**
  - Grid `28px 170px 130px 1fr 80px 150px`: checkbox (header checkbox supports indeterminate), Server, Map, File (mono, ellipsis), Size (right-aligned), Modified.
  - Selected rows get a cyan tint.
- **Sticky selection bar:**
  - "3 selected", then muted "642 KB · lna-theisland, lna-center, omega-astraeos".
  - Buttons: "Clear selection" (ghost) and "Delete 3 files" (destructive outline). The count updates with the selection; the button is disabled at 0.
  - Clicking delete opens the existing confirm, restyled.
- Empty states:
  - Before any search: "Enter an ID and search to find files."
  - No results: "No files found for this ID".

### 1l. Settings (`Screen - Settings.dc.html`)
- Description: "Stored in %LOCALAPPDATA%\arkade\manager\config.json" (path in mono).
- Layout: a 168px in-page section nav on the left (anchor links; active item bg `rgba(255,255,255,.05)`), with the sections stacked on the right and separated by hairlines (26px vertical padding). No cards.
  1. **Server directories.**
     - Panel with one row per game, grid `200px 1fr 80px 120px`: game colour square + name, mono path input, Browse, and a live check.
     - Check states: "6 servers found" in green, or "Folder not found" in yellow with the input border at `rgba(251,191,36,.45)`.
     - "Save" (primary) sits at the top right of the section.
  2. **Monthly archive.** Destination path with Browse, and the five cluster folder names as mono tags.
  3. **ARK maps.**
     - Editable table: Display name, ID (mono), Saves folder (mono), Map file (mono), and a remove ×.
     - Section actions: Reset defaults (ghost), Add map (secondary), Save (primary).
  4. **Appearance.** Theme segmented control: Dark / Light.
  5. **Updates.**
     - Line: "v2.6.1 · released Oct 27, 2026 · Up to date" (green). When an update is available, show "Install update" as primary.
     - Buttons: Release notes (ghost), Check now (secondary).
- Keep the inline `settings-message` feedback below each section's Save.

---

## Interactions and behavior
- **Group collapse (Backups):** clicking a group header toggles it. Persist collapsed group keys in `localStorage` (`arkade.collapsedGroups`). "Collapse all" flips to "Expand all" once every group is collapsed. Filters still hide empty groups (`applyJobFilters`).
- **Relative times:** "in 12 min" and "18 min ago" are calculated on the client and refreshed on the existing 2s status tick.
- **Running row:** driven by `status.current_job` and `backup_progress`. The row's next-save cell shows the progress bar until `job_updated` arrives.
- **Hover states:**
  - Table rows: `rgba(255,255,255,.022)`.
  - Group rows: `#0D131D`.
  - Secondary buttons: border `rgba(255,255,255,.14)`, bg `#081B33`.
  - Primary button: `#C42BD4`.
  - Nav items: `rgba(255,255,255,.05)`.
- **Focus:** cyan ring on all inputs and buttons (`box-shadow: 0 0 0 2px rgba(31,227,245,.45)`).
- **Transitions:** 120–200ms ease for backgrounds and the drawer. No looping animations.

## State to add
- `state.collapsedGroups: Set<string>`
- `state.logFilter = { level, source, text, follow }`
- `state.pluginToggle = { server, pending: Map }`
- `state.pluginInstall = { sourceSel: Set, destSel: Set }`; the review list is derived from these.
- `state.lookupSelection: Set<path>`
- `state.editJobId` (drawer open when not null)

## Assets
- `assets/Icon_ArkadeLogo.png`, copied from the repo's `public/Icon_ArkadeLogo.png`.
- Icons are inline 16px stroke SVGs (stroke-width 1.8–2.4), as in the current `index.html`.
- Fonts: Google Fonts, Manrope and JetBrains Mono. For an offline Tauri build, bundle both locally.

## Files
`screenshots/` (1280×800 at 1x; Settings is 1280×1180):
- `01-backups.png`
- `02-edit-job.png`
- `03-add-job.png`
- `04-monthly-status.png`
- `05-logs.png`
- `06-plugins-install.png`
- `07-plugins-toggle.png`
- `08-data-lookup.png`
- `09-settings.png`

`designs/`:
- `ARKADE Manager Redesign.dc.html`: canvas with every screen.
- `ArkSidebar.dc.html`: shared sidebar.
- `Screen - Backups.dc.html`
- `Screen - Edit Job.dc.html`
- `Screen - Add Job.dc.html`
- `Screen - Monthly Status.dc.html`
- `Screen - Logs.dc.html`
- `Screen - Plugins Install A.dc.html`
- `Screen - Plugins Toggle A.dc.html`
- `Screen - Data Lookup A.dc.html`
- `Screen - Settings.dc.html`
- `support.js`: runtime for opening the `.dc.html` files.

Repo files to change: `index.html`, `src/styles/base.css`, `layout.css`, `components.css`, `tables.css`, `forms.css`, `modals.css`, `plugins.css`, `src/ui/jobs/table.js`, `jobs/filters.js`, `jobs/form.js`, `wizard/backup-wizard.js`, `logs.js`, `plugins/install.js`, `plugins/toggle.js`, `data-lookup.js`, `settings/*.js`, `init.js` (tab ids: merge `new-plugins` and `plugin-toggle` into `plugins`).
