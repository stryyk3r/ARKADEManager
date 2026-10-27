import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { state, GAMES, getJobGroupKey, gameChipHtml } from '../../state.js';
import { escapeHtml, setSelectValue, populateMapSelect, getSwitch, setSwitch, bindSwitch, setButtonBusy } from '../../utils/dom.js';
import { formatFileSize, formatTime, formatUntil, shortenPath, joinPath } from '../../utils/format.js';
import { openOverlay, closeOverlay, confirmDialog, toast, toastError } from '../feedback.js';
import { refreshMapSelects } from '../settings/maps.js';
import { refreshJobs } from './table.js';

const $ = (id) => document.getElementById(id);

const ENABLED_HINTS = {
  ark: 'RCON SaveWorld runs first when enabled in GameUserSettings.ini',
  minecraft: 'Saving pauses over RCON while the world is copied',
  palworld: 'A REST API save runs first, using PalWorldSettings.ini',
};

let drawerSnapshot = null;

// ---------- Shared job actions ----------

/** The editable fields of a saved job, as `update_job` expects them. */
export function jobToInput(job) {
  return {
    id: job.id,
    job_type: job.job_type || 'ark',
    monthly_cluster: job.monthly_cluster,
    name: job.name,
    root_dir: job.root_dir,
    destination_dir: job.destination_dir,
    map: job.map || '',
    include_saves: !!job.include_saves,
    include_map: !!job.include_map,
    include_server_files: !!job.include_server_files,
    include_plugin_configs: !!job.include_plugin_configs,
    interval_value: job.interval_value,
    interval_unit: job.interval_unit,
    retention_days: job.retention_days,
    enabled: !!job.enabled,
    rcon_host: job.rcon_host ?? null,
    rcon_port: job.rcon_port ?? null,
    rcon_password: job.rcon_password ?? null,
  };
}

export async function runJobNow(jobId) {
  const job = state.allJobs.find(j => j.id === jobId);
  try {
    await invoke('run_job_now', { id: jobId });
    toast(`${job?.name || 'Job'} queued`, { kind: 'success', detail: 'It starts when the scheduler is free.' });
  } catch (e) {
    toastError('Could not run the job', e);
  }
}

export async function openBackupLocation(destinationDir) {
  if (!destinationDir || !destinationDir.trim()) {
    toast('No backup folder is set for this job', { kind: 'warning' });
    return;
  }
  try {
    await invoke('open_backup_location', { path: destinationDir });
  } catch (e) {
    toastError('Could not open the backup folder', e);
  }
}

export async function setJobEnabled(jobId, enabled) {
  const job = state.allJobs.find(j => j.id === jobId);
  if (!job) return;
  try {
    await invoke('update_job', { job: { ...jobToInput(job), enabled } });
    await refreshJobs();
    toast(enabled ? `Resumed ${job.name}` : `Paused ${job.name}`, { kind: 'success' });
  } catch (e) {
    toastError(enabled ? 'Could not resume the schedule' : 'Could not pause the schedule', e);
  }
}

/** Ask, then delete. Resolves true when the job was deleted. */
export async function deleteJob(jobId) {
  const job = state.allJobs.find(j => j.id === jobId);
  if (!job) return false;
  const confirmed = await confirmDialog({
    title: `Delete ${job.name}?`,
    body: `<p>The job and its schedule are removed. Backups already saved in
      <span class="mono">${escapeHtml(job.destination_dir)}</span> are kept.</p>`,
    confirmLabel: 'Delete job',
    danger: true,
  });
  if (!confirmed) return false;
  try {
    await invoke('delete_job', { id: jobId });
    await refreshJobs();
    toast(`Deleted ${job.name}`, { kind: 'success' });
    return true;
  } catch (e) {
    toastError('Could not delete the job', e);
    return false;
  }
}

export async function pickDirectory(title, defaultPath) {
  try {
    const selected = await open({ directory: true, multiple: false, title, defaultPath: defaultPath || undefined });
    return typeof selected === 'string' ? selected : null;
  } catch (e) {
    toastError('Could not open the folder picker', e);
    return null;
  }
}

// ---------- Edit drawer ----------

function drawerType() {
  return state.allJobs.find(j => j.id === state.editJobId)?.job_type || 'ark';
}

function showFieldsForType(type) {
  document.querySelectorAll('#editJobDrawer [data-for-type]').forEach(el => {
    el.hidden = !el.dataset.forType.split(' ').includes(type);
  });
  $('drawerNameRow').classList.toggle('field-row', type === 'ark');
}

export function updateDrawerDerivedPath() {
  const type = drawerType();
  const root = $('drawerRootDir').value.trim();
  const hint = $('drawerDerivedPath');
  if (!root) {
    hint.textContent = '';
    return;
  }
  let label = '';
  let path = '';
  if (type === 'ark') {
    const map = state.arkMaps.find(m => m.id === $('drawerMap').value);
    if (map) {
      label = 'saves';
      path = joinPath(root, 'ShooterGame', 'Saved', 'SavedArks', map.folder_name);
    }
  } else if (type === 'minecraft') {
    label = 'world';
    path = joinPath(root, 'world');
  } else if (type === 'palworld') {
    label = 'saves';
    path = joinPath(root, 'Pal', 'Saved', 'SaveGames', '0');
  }
  hint.textContent = path ? `${label} → ${shortenPath(path, 4)}` : '';
  hint.title = path;
}

function clearDrawerErrors() {
  ['drawerJobNameError', 'drawerMapError', 'drawerRconError', 'drawerRootDirError', 'drawerDestinationDirError']
    .forEach(id => { $(id).textContent = ''; });
  $('drawerSaveError').hidden = true;
}

function drawerMetaText(job) {
  const parts = [];
  parts.push(job.last_run_at ? `Last save ${formatTime(job.last_run_at)}` : 'Never saved');
  if (job.last_file_size) parts.push(formatFileSize(job.last_file_size));
  if (!job.enabled) parts.push('paused');
  else if (job.next_run_at) parts.push(`next ${formatUntil(job.next_run_at)}`);
  return parts.join(' · ');
}

export async function openEditDrawer(jobId) {
  let job = state.allJobs.find(j => j.id === jobId);
  if (!job) {
    await refreshJobs();
    job = state.allJobs.find(j => j.id === jobId);
  }
  if (!job) {
    toast('That job no longer exists', { kind: 'warning' });
    return;
  }
  state.editJobId = job.id;
  const type = job.job_type || 'ark';
  const groupKey = getJobGroupKey(job);

  $('drawerGame').className = `drawer-game game-${groupKey}`;
  $('drawerGame').innerHTML = `${gameChipHtml(groupKey, true)}${escapeHtml(GAMES[groupKey].label)}`;
  $('drawerJobTitle').textContent = job.name;
  $('drawerJobMeta').textContent = drawerMetaText(job);

  $('drawerJobName').value = job.name || '';
  $('drawerIntervalValue').value = job.interval_value || 1;
  setSelectValue($('drawerIntervalUnit'), job.interval_unit || 'minutes');
  $('drawerRetention').value = job.retention_days || 7;
  setSelectValue($('drawerCluster'), job.monthly_cluster || GAMES[groupKey].cluster);
  $('drawerRootDir').value = job.root_dir || '';
  $('drawerDestinationDir').value = job.destination_dir || '';
  setSwitch($('drawerEnabled'), job.enabled);
  $('drawerEnabledHint').textContent = ENABLED_HINTS[type] || '';

  if (type === 'ark') {
    if (state.arkMaps.length === 0) await refreshMapSelects(false);
    populateMapSelect($('drawerMap'), state.arkMaps, job.map || '');
    $('drawerIncludeMap').checked = !!job.include_map;
    $('drawerIncludeSaves').checked = !!job.include_saves;
    $('drawerIncludeServerFiles').checked = !!job.include_server_files;
    $('drawerIncludePluginConfigs').checked = !!job.include_plugin_configs;
  } else if (type === 'minecraft') {
    $('drawerRconHost').value = job.rcon_host || '';
    $('drawerRconPort').value = job.rcon_port || 25575;
    $('drawerRconPassword').value = job.rcon_password || '';
  } else if (type === 'palworld') {
    $('drawerApiHost').value = job.rcon_host || '';
  }

  showFieldsForType(type);
  updateDrawerDerivedPath();
  clearDrawerErrors();
  $('editJobForm').scrollTop = 0;
  drawerSnapshot = JSON.stringify(collectDrawerData(job, { validate: false }));
  openOverlay($('editJobDrawer'), { onDismiss: closeEditDrawer, initialFocus: '#drawerJobName' });
}

/** Close the drawer, asking first if there are unsaved edits (unless `force`). */
export async function closeEditDrawer(force = false) {
  const job = state.allJobs.find(j => j.id === state.editJobId);
  if (!force && job && drawerSnapshot && JSON.stringify(collectDrawerData(job, { validate: false })) !== drawerSnapshot) {
    const discard = await confirmDialog({
      title: 'Discard changes?',
      body: `<p>Your edits to ${escapeHtml(job.name)} have not been saved.</p>`,
      confirmLabel: 'Discard',
      danger: true,
    });
    if (!discard) return;
  }
  closeOverlay($('editJobDrawer'));
  state.editJobId = null;
  drawerSnapshot = null;
}

function collectDrawerData(job, { validate = true } = {}) {
  const type = job.job_type || 'ark';
  const name = $('drawerJobName').value.trim();
  const rootDir = $('drawerRootDir').value.trim();
  const destinationDir = $('drawerDestinationDir').value.trim();
  const intervalValue = parseInt($('drawerIntervalValue').value, 10);
  const retention = parseInt($('drawerRetention').value, 10);

  if (validate) {
    clearDrawerErrors();
    let valid = true;
    const fail = (id, message) => {
      $(id).textContent = message;
      valid = false;
    };
    if (!name) fail('drawerJobNameError', 'Job name is required');
    if (!rootDir) fail('drawerRootDirError', 'Server root is required');
    if (!destinationDir) fail('drawerDestinationDirError', 'Destination is required');
    if (type === 'ark' && !$('drawerMap').value) fail('drawerMapError', 'Pick a map');
    if (type === 'minecraft') {
      const port = parseInt($('drawerRconPort').value, 10);
      if (!$('drawerRconHost').value.trim()) fail('drawerRconError', 'RCON host is required');
      else if (!port || port < 1 || port > 65535) fail('drawerRconError', 'RCON port must be 1–65535');
      else if (!$('drawerRconPassword').value) fail('drawerRconError', 'RCON password is required');
    }
    if (!valid) return null;
  }

  const payload = {
    ...jobToInput(job),
    name,
    root_dir: rootDir,
    destination_dir: destinationDir,
    monthly_cluster: $('drawerCluster').value,
    interval_value: Math.max(1, intervalValue || 1),
    interval_unit: $('drawerIntervalUnit').value,
    enabled: getSwitch($('drawerEnabled')),
  };

  if (type === 'ark') {
    Object.assign(payload, {
      map: $('drawerMap').value,
      retention_days: Math.max(1, retention || 7),
      include_map: $('drawerIncludeMap').checked,
      include_saves: $('drawerIncludeSaves').checked,
      include_server_files: $('drawerIncludeServerFiles').checked,
      include_plugin_configs: $('drawerIncludePluginConfigs').checked,
    });
  } else if (type === 'minecraft') {
    Object.assign(payload, {
      map: '',
      retention_days: 30,
      rcon_host: $('drawerRconHost').value.trim(),
      rcon_port: parseInt($('drawerRconPort').value, 10) || 25575,
      rcon_password: $('drawerRconPassword').value,
    });
  } else if (type === 'palworld') {
    Object.assign(payload, {
      map: '',
      retention_days: Math.max(1, retention || 7),
      rcon_host: $('drawerApiHost').value.trim() || null,
      rcon_port: null,
      rcon_password: null,
    });
  }
  return payload;
}

export async function saveJobChanges() {
  const job = state.allJobs.find(j => j.id === state.editJobId);
  if (!job) return;
  const payload = collectDrawerData(job);
  if (!payload) return;

  const btn = $('drawerSaveBtn');
  setButtonBusy(btn, true, 'Saving…');
  try {
    await invoke('update_job', { job: payload });
    await refreshJobs();
    closeEditDrawer(true);
    toast(`Saved ${payload.name}`, { kind: 'success' });
  } catch (e) {
    const err = $('drawerSaveError');
    err.textContent = String(e);
    err.hidden = false;
  } finally {
    setButtonBusy(btn, false);
  }
}

export async function deleteJobFromDrawer() {
  if (!state.editJobId) return;
  if (await deleteJob(state.editJobId)) closeEditDrawer(true);
}

export async function pickDrawerRootDir() {
  const selected = await pickDirectory('Select server root folder', $('drawerRootDir').value.trim());
  if (selected) {
    $('drawerRootDir').value = selected;
    $('drawerRootDirError').textContent = '';
    updateDrawerDerivedPath();
  }
}

export async function pickDrawerDestinationDir() {
  const selected = await pickDirectory('Select backup destination folder', $('drawerDestinationDir').value.trim());
  if (selected) {
    $('drawerDestinationDir').value = selected;
    $('drawerDestinationDirError').textContent = '';
  }
}

export function setupEditDrawer() {
  bindSwitch($('drawerEnabled'));
  $('drawerRootDir').addEventListener('input', updateDrawerDerivedPath);
  $('drawerMap').addEventListener('change', updateDrawerDerivedPath);
  $('editJobForm').addEventListener('submit', (e) => {
    e.preventDefault();
    saveJobChanges();
  });
  $('editJobForm').addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && e.target.tagName === 'INPUT') {
      e.preventDefault();
      saveJobChanges();
    }
  });
}
