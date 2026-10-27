import { invoke } from '@tauri-apps/api/core';
import { state, GAMES, DEFAULT_ASA_SERVER_ROOT, getJobGroupKey } from '../../state.js';
import {
  escapeHtml, setSelectValue, populateMapSelect, getSwitch, setSwitch, bindSwitch, debounce, setButtonBusy,
} from '../../utils/dom.js';
import { basename, formatBackupStamp, formatFileSize, formatInterval, joinPath, shortenPath, normalizePathKey } from '../../utils/format.js';
import { openOverlay, closeOverlay, toast } from '../feedback.js';
import { pickDirectory } from '../jobs/form.js';
import { refreshJobs } from '../jobs/table.js';
import { refreshMapSelects } from '../settings/maps.js';

const $ = (id) => document.getElementById(id);

const QUESTIONS = {
  game: 'Which game is this server?',
  root: 'Where is the server installed?',
  destination: 'Where should backups go?',
  map: 'Which map does it run?',
  rcon: 'How do we reach RCON?',
  api: 'Where is the REST API?',
  schedule: 'Name and schedule',
};

/** Folder each game's server root should contain. */
const ROOT_MARKERS = {
  ark: { folder: 'ShooterGame', found: 'ARK server files found' },
  minecraft: { folder: 'world', found: 'Minecraft world found' },
  palworld: { folder: 'Pal', found: 'Palworld server files found' },
};

let rootCheck = null;
let destinationCheck = null;

function jobType() {
  return GAMES[state.wizard?.game]?.jobType ?? null;
}

function stepsFor(game) {
  const type = GAMES[game]?.jobType;
  const fourth = type === 'minecraft'
    ? { key: 'rcon', name: 'RCON' }
    : type === 'palworld'
      ? { key: 'api', name: 'REST API' }
      : { key: 'map', name: 'Map' };
  return [
    { key: 'game', name: 'Game' },
    { key: 'root', name: 'Server root' },
    { key: 'destination', name: 'Destination' },
    fourth,
    { key: 'schedule', name: 'Schedule' },
  ];
}

function currentSteps() {
  return stepsFor(state.wizard.game);
}

function currentStepKey() {
  return currentSteps()[state.wizard.step].key;
}

function suggestedName() {
  return basename($('wizardRootDir').value.trim()).replace(/[<>:"/\\|?*]/g, '_').replace(/[ .]+$/, '');
}

function stepValue(key) {
  const w = state.wizard;
  switch (key) {
    case 'game':
      return w.game ? { text: GAMES[w.game].label } : null;
    case 'root':
      return $('wizardRootDir').value.trim() ? { text: $('wizardRootDir').value.trim(), mono: true } : null;
    case 'destination':
      return $('wizardDestinationDir').value.trim() ? { text: $('wizardDestinationDir').value.trim(), mono: true } : null;
    case 'map': {
      const map = state.arkMaps.find(m => m.id === $('wizardMapSelect').value);
      return map ? { text: map.display_name } : null;
    }
    case 'rcon': {
      const host = $('wizardMinecraftRconHost').value.trim();
      return host ? { text: `${host}:${$('wizardMinecraftRconPort').value || 25575}`, mono: true } : null;
    }
    case 'api':
      return { text: $('wizardPalworldApiHost').value.trim() || '127.0.0.1 (default)', mono: true };
    case 'schedule':
      return { text: `Every ${formatInterval($('wizardIntervalValue').value, $('wizardIntervalUnit').value)}` };
    default:
      return null;
  }
}

function renderRail() {
  const steps = currentSteps();
  const cur = state.wizard.step;
  $('wizardStepper').innerHTML = steps.map((s, i) => {
    const status = i < cur ? 'done' : i === cur ? 'current' : 'upcoming';
    const value = i <= cur ? stepValue(s.key) : null;
    return `
      <li class="step ${status}" ${status === 'done' ? `data-goto-step="${i}"` : ''}>
        <div class="step-marker"><span class="step-dot">${status === 'done' ? '✓' : i + 1}</span><span class="step-line"></span></div>
        <div class="step-text">
          <div class="step-name">${escapeHtml(s.name)}</div>
          <div class="step-value ${value?.mono ? 'mono' : ''}" title="${escapeHtml(value?.text || '')}">${escapeHtml(value?.text || '')}</div>
        </div>
      </li>`;
  }).join('');
}

function render() {
  const steps = currentSteps();
  const step = steps[state.wizard.step];
  document.querySelectorAll('#addJobModal .wizard-step').forEach(el => {
    el.classList.toggle('active', el.dataset.step === step.key);
  });
  $('wizardCount').textContent = `Step ${state.wizard.step + 1} of ${steps.length}`;
  $('wizardQuestion').textContent = QUESTIONS[step.key];
  $('wizardPrevBtn').hidden = state.wizard.step === 0;
  const next = steps[state.wizard.step + 1];
  $('wizardNextBtn').textContent = next ? `Next: ${next.name}` : 'Create job';
  renderRail();
}

function focusStep() {
  requestAnimationFrame(() => {
    const active = document.querySelector('#addJobModal .wizard-step.active');
    const target = active?.querySelector('.option-card.selected, .option-card, input:not([type="checkbox"]), .select-trigger');
    target?.focus({ preventScroll: true });
  });
}

// ---------- Step: game ----------

function selectGame(game) {
  const previousType = jobType();
  state.wizard.game = game;
  document.querySelectorAll('#wizardGameOptions .option-card').forEach(card => {
    const on = card.dataset.game === game;
    card.classList.toggle('selected', on);
    card.setAttribute('aria-checked', String(on));
  });
  $('wizardBackupTypeError').textContent = '';
  setSelectValue($('wizardMonthlyCluster'), GAMES[game].cluster);
  const type = GAMES[game].jobType;
  $('wizardRetentionField').hidden = type === 'minecraft';
  $('wizardRetentionNote').hidden = type !== 'minecraft';
  $('wizardRootDir').placeholder = type === 'ark'
    ? 'C:\\arkservers\\asaservers\\my-server'
    : type === 'minecraft' ? 'C:\\minecraftservers\\my-server' : 'C:\\palworldservers\\my-server';
  if (previousType && previousType !== type) {
    rootCheck = null;
    $('wizardRootCheck').innerHTML = '';
  }
  renderRail();
}

// ---------- Step: root ----------

function rootParentFor(game) {
  const config = state.config || {};
  if (GAMES[game]?.jobType === 'ark') return config.asa_server_root || DEFAULT_ASA_SERVER_ROOT;
  if (game === 'minecraft') return config.minecraft_server_root || '';
  if (game === 'palworld') return config.palworld_server_root || '';
  return '';
}

async function loadRootSuggestions() {
  const parent = rootParentFor(state.wizard.game);
  const box = $('wizardRootSuggestions');
  if (!parent) {
    box.hidden = true;
    return;
  }
  try {
    const info = await invoke('inspect_directory', { path: parent });
    const used = new Set(state.allJobs.map(j => normalizePathKey(j.root_dir)));
    const children = (info.child_dirs || []).map(name => joinPath(parent, name));
    if (!info.exists || children.length === 0) {
      box.hidden = true;
      return;
    }
    $('wizardRootSuggestionsLabel').textContent = `Servers under ${parent}`;
    $('wizardRootSuggestionList').innerHTML = children
      .sort((a, b) => Number(used.has(normalizePathKey(a))) - Number(used.has(normalizePathKey(b))))
      .map(path => {
        const inUse = used.has(normalizePathKey(path));
        return `<button class="pick-item" type="button" data-pick-root="${escapeHtml(path)}" title="${escapeHtml(path)}">${escapeHtml(basename(path))}${inUse ? ' <span style="color:var(--text-muted)">· has a job</span>' : ''}</button>`;
      })
      .join('');
    box.hidden = false;
  } catch {
    box.hidden = true;
  }
}

const checkRoot = debounce(async () => {
  const path = $('wizardRootDir').value.trim();
  const line = $('wizardRootCheck');
  rootCheck = null;
  if (!path) {
    line.innerHTML = '';
    return;
  }
  try {
    const info = await invoke('inspect_directory', { path });
    if (path !== $('wizardRootDir').value.trim()) return;
    const marker = ROOT_MARKERS[jobType()];
    const hasMarker = marker && (info.child_dirs || []).some(d => d.toLowerCase() === marker.folder.toLowerCase());
    rootCheck = { exists: info.exists, hasMarker };
    if (!info.exists) line.innerHTML = '<span class="bad">Folder not found</span>';
    else if (hasMarker) line.innerHTML = `<span class="ok">${escapeHtml(marker.found)}</span>`;
    else line.innerHTML = `<span class="warn">No ${escapeHtml(marker?.folder || '')} folder here.</span><span>Pick the server's root folder.</span>`;
  } catch {
    line.innerHTML = '';
  }
}, 250);

function onRootChanged() {
  rootCheck = null;
  $('wizardRootDirError').textContent = '';
  if (state.wizard.autoName || !$('wizardJobName').value.trim()) {
    $('wizardJobName').value = suggestedName();
    state.wizard.autoName = true;
  }
  checkRoot();
  updateFilePreview();
  renderRail();
}

// ---------- Step: destination ----------

function updateFilePreview() {
  const name = $('wizardJobName').value.trim() || suggestedName() || 'JobName';
  const ext = jobType() === 'minecraft' ? '7z' : 'zip';
  $('wizardFilePreview').innerHTML = `${escapeHtml(name)}_<span class="stamp">${formatBackupStamp()}</span>.${ext}`
    + (ext === '7z' ? ' <span style="color:var(--text-muted)">(.zip without 7-Zip)</span>' : '');
}

function renderRecentDestinations() {
  const seen = new Set();
  const current = normalizePathKey($('wizardDestinationDir').value);
  const recent = [...state.allJobs]
    .sort((a, b) => (b.last_run_at || '').localeCompare(a.last_run_at || ''))
    .map(j => j.destination_dir)
    .filter(path => {
      const key = normalizePathKey(path);
      if (!path || seen.has(key) || key === current) return false;
      seen.add(key);
      return true;
    })
    .slice(0, 4);
  $('wizardRecentDestinations').hidden = recent.length === 0;
  $('wizardRecentDestinationList').innerHTML = recent
    .map(path => `<button class="pick-item" type="button" data-pick-destination="${escapeHtml(path)}" title="${escapeHtml(path)}">${escapeHtml(path)}</button>`)
    .join('');
}

const checkDestination = debounce(async () => {
  const path = $('wizardDestinationDir').value.trim();
  const line = $('wizardDestinationCheck');
  destinationCheck = null;
  if (!path) {
    line.innerHTML = '';
    return;
  }
  try {
    const check = await invoke('check_destination', { path });
    if (path !== $('wizardDestinationDir').value.trim()) return;
    destinationCheck = check;
    const parts = [];
    if (!check.exists) parts.push('<span class="bad">Folder does not exist</span>');
    else if (check.writable === true) parts.push('<span class="ok">Writable</span>');
    else if (check.writable === false) parts.push('<span class="bad">Not writable</span>');
    if (check.free_bytes != null) parts.push(`<span>${escapeHtml(formatFileSize(check.free_bytes))} free${check.volume ? ` on ${escapeHtml(check.volume)}` : ''}</span>`);
    if (check.same_volume_as_monthly === true) parts.push('<span>Same drive as the monthly archive, monthly copies are hard links</span>');
    else if (check.same_volume_as_monthly === false) parts.push('<span>Monthly copies go to another drive (full copies)</span>');
    line.innerHTML = parts.join('');
  } catch (e) {
    line.innerHTML = `<span class="bad">${escapeHtml(String(e))}</span>`;
  }
}, 300);

function onDestinationChanged() {
  destinationCheck = null;
  $('wizardDestinationDirError').textContent = '';
  checkDestination();
  renderRecentDestinations();
  renderRail();
}

// ---------- Step: map ----------

function updateWizardDerivedPath() {
  const map = state.arkMaps.find(m => m.id === $('wizardMapSelect').value);
  const root = $('wizardRootDir').value.trim();
  const hint = $('wizardDerivedPath');
  const path = map && root ? joinPath(root, 'ShooterGame', 'Saved', 'SavedArks', map.folder_name) : '';
  hint.textContent = path ? `saves → ${shortenPath(path, 4)}` : '';
  hint.title = path;
}

// ---------- Validation ----------

function validateStep(key) {
  const fail = (id, message) => {
    $(id).textContent = message;
    return false;
  };
  switch (key) {
    case 'game':
      return state.wizard.game ? true : fail('wizardBackupTypeError', 'Pick a game to continue');
    case 'root': {
      const root = $('wizardRootDir').value.trim();
      if (!root) return fail('wizardRootDirError', 'Choose the server folder');
      if (rootCheck && !rootCheck.exists) return fail('wizardRootDirError', 'That folder does not exist');
      return true;
    }
    case 'destination': {
      const dest = $('wizardDestinationDir').value.trim();
      if (!dest) return fail('wizardDestinationDirError', 'Choose where backups go');
      if (destinationCheck && !destinationCheck.exists) return fail('wizardDestinationDirError', 'Create this folder first, or pick another');
      if (destinationCheck && destinationCheck.writable === false) return fail('wizardDestinationDirError', 'ARKADE Manager cannot write to this folder');
      return true;
    }
    case 'map':
      return $('wizardMapSelect').value ? true : fail('wizardMapError', 'Pick a map');
    case 'rcon': {
      let ok = true;
      const port = parseInt($('wizardMinecraftRconPort').value, 10);
      if (!$('wizardMinecraftRconHost').value.trim()) ok = fail('wizardMinecraftRconHostError', 'RCON host is required');
      if (!port || port < 1 || port > 65535) ok = fail('wizardMinecraftRconPortError', 'Port must be 1–65535');
      if (!$('wizardMinecraftRconPassword').value) ok = fail('wizardMinecraftRconPasswordError', 'RCON password is required');
      return ok;
    }
    case 'schedule': {
      let ok = true;
      if (!$('wizardJobName').value.trim()) ok = fail('wizardJobNameError', 'Job name is required');
      if (!$('wizardMonthlyCluster').value) ok = fail('wizardMonthlyClusterError', 'Pick a cluster');
      return ok;
    }
    default:
      return true;
  }
}

function clearWizardErrors() {
  document.querySelectorAll('#addJobModal .field-error').forEach(el => { el.textContent = ''; });
  $('wizardSaveError').hidden = true;
}

// ---------- Navigation ----------

export async function showAddJobForm() {
  state.wizard = { step: 0, game: null, autoName: true };
  resetWizard();
  if (state.arkMaps.length === 0) await refreshMapSelects(false);
  populateMapSelect($('wizardMapSelect'), state.arkMaps, '');
  render();
  openOverlay($('addJobModal'), { onDismiss: closeAddJobModal });
  focusStep();
}

export function closeAddJobModal() {
  closeOverlay($('addJobModal'));
}

export function resetWizard() {
  rootCheck = null;
  destinationCheck = null;
  document.querySelectorAll('#wizardGameOptions .option-card').forEach(card => {
    card.classList.remove('selected');
    card.setAttribute('aria-checked', 'false');
  });
  ['wizardRootDir', 'wizardDestinationDir', 'wizardJobName', 'wizardMinecraftRconPassword', 'wizardPalworldApiHost']
    .forEach(id => { $(id).value = ''; });
  $('wizardMinecraftRconHost').value = '127.0.0.1';
  $('wizardMinecraftRconPort').value = '25575';
  $('wizardIntervalValue').value = '30';
  setSelectValue($('wizardIntervalUnit'), 'minutes');
  $('wizardRetentionDays').value = '7';
  setSwitch($('wizardEnabled'), true);
  $('wizardIncludeMap').checked = true;
  $('wizardIncludeSaves').checked = true;
  $('wizardIncludeServerFiles').checked = true;
  $('wizardIncludePluginConfigs').checked = false;
  $('wizardRootCheck').innerHTML = '';
  $('wizardDestinationCheck').innerHTML = '';
  $('wizardRootSuggestions').hidden = true;
  $('wizardDerivedPath').textContent = '';
  clearWizardErrors();
}

function goToStep(index) {
  state.wizard.step = index;
  const key = currentStepKey();
  if (key === 'root') loadRootSuggestions();
  if (key === 'destination') {
    renderRecentDestinations();
    updateFilePreview();
  }
  if (key === 'map') updateWizardDerivedPath();
  if (key === 'schedule' && !$('wizardJobName').value.trim()) {
    $('wizardJobName').value = suggestedName();
    state.wizard.autoName = true;
  }
  render();
  focusStep();
}

export function wizardNextStep() {
  clearWizardErrors();
  const key = currentStepKey();
  if (!validateStep(key)) return;
  if (key === 'schedule') {
    wizardFinish();
    return;
  }
  goToStep(state.wizard.step + 1);
}

export function wizardPreviousStep() {
  if (state.wizard.step === 0) return;
  clearWizardErrors();
  goToStep(state.wizard.step - 1);
}

export async function pickWizardRootDir() {
  const gameName = GAMES[state.wizard?.game]?.label || 'server';
  const selected = await pickDirectory(`Select the ${gameName} server folder`, $('wizardRootDir').value.trim() || rootParentFor(state.wizard?.game));
  if (selected) {
    $('wizardRootDir').value = selected;
    onRootChanged();
  }
}

export async function pickWizardDestinationDir() {
  const selected = await pickDirectory('Select backup destination folder', $('wizardDestinationDir').value.trim());
  if (selected) {
    $('wizardDestinationDir').value = selected;
    onDestinationChanged();
  }
}

function collectWizardJob() {
  const type = jobType();
  const base = {
    job_type: type,
    monthly_cluster: $('wizardMonthlyCluster').value,
    name: $('wizardJobName').value.trim(),
    root_dir: $('wizardRootDir').value.trim(),
    destination_dir: $('wizardDestinationDir').value.trim(),
    map: '',
    include_saves: false,
    include_map: false,
    include_server_files: false,
    include_plugin_configs: false,
    interval_value: Math.max(1, parseInt($('wizardIntervalValue').value, 10) || 1),
    interval_unit: $('wizardIntervalUnit').value,
    retention_days: Math.max(1, parseInt($('wizardRetentionDays').value, 10) || 7),
    enabled: getSwitch($('wizardEnabled')),
    rcon_host: null,
    rcon_port: null,
    rcon_password: null,
  };
  if (type === 'minecraft') {
    return {
      ...base,
      retention_days: 30,
      rcon_host: $('wizardMinecraftRconHost').value.trim(),
      rcon_port: parseInt($('wizardMinecraftRconPort').value, 10) || 25575,
      rcon_password: $('wizardMinecraftRconPassword').value,
    };
  }
  if (type === 'palworld') {
    return { ...base, rcon_host: $('wizardPalworldApiHost').value.trim() || null };
  }
  return {
    ...base,
    map: $('wizardMapSelect').value,
    include_saves: $('wizardIncludeSaves').checked,
    include_map: $('wizardIncludeMap').checked,
    include_server_files: $('wizardIncludeServerFiles').checked,
    include_plugin_configs: $('wizardIncludePluginConfigs').checked,
  };
}

export async function wizardFinish() {
  if (!validateStep('schedule')) return;
  const job = collectWizardJob();
  const btn = $('wizardNextBtn');
  setButtonBusy(btn, true, 'Creating…');
  try {
    const created = await invoke('add_job', { job });
    closeAddJobModal();
    // Make sure the new job's group is visible.
    state.collapsedGroups.delete(getJobGroupKey(created || job));
    await refreshJobs();
    toast(`Created ${job.name}`, {
      kind: 'success',
      detail: job.enabled ? `Every ${formatInterval(job.interval_value, job.interval_unit)}, starting now` : 'Saved paused',
    });
  } catch (e) {
    const err = $('wizardSaveError');
    err.textContent = String(e);
    err.hidden = false;
  } finally {
    setButtonBusy(btn, false);
  }
}

export function initWizard() {
  $('wizardGameOptions').addEventListener('click', (e) => {
    const card = e.target.closest('.option-card');
    if (card) selectGame(card.dataset.game);
  });
  $('wizardGameOptions').addEventListener('dblclick', (e) => {
    if (e.target.closest('.option-card')) wizardNextStep();
  });
  $('wizardRootDir').addEventListener('input', onRootChanged);
  $('wizardDestinationDir').addEventListener('input', onDestinationChanged);
  $('wizardJobName').addEventListener('input', () => {
    state.wizard.autoName = false;
    $('wizardJobNameError').textContent = '';
  });
  $('wizardMapSelect').addEventListener('change', () => {
    $('wizardMapError').textContent = '';
    updateWizardDerivedPath();
    renderRail();
  });
  ['wizardIntervalValue', 'wizardMinecraftRconHost', 'wizardMinecraftRconPort', 'wizardPalworldApiHost']
    .forEach(id => $(id).addEventListener('input', renderRail));
  $('wizardIntervalUnit').addEventListener('change', renderRail);
  bindSwitch($('wizardEnabled'));

  $('wizardRootSuggestionList').addEventListener('click', (e) => {
    const item = e.target.closest('[data-pick-root]');
    if (!item) return;
    $('wizardRootDir').value = item.dataset.pickRoot;
    onRootChanged();
  });
  $('wizardRecentDestinationList').addEventListener('click', (e) => {
    const item = e.target.closest('[data-pick-destination]');
    if (!item) return;
    $('wizardDestinationDir').value = item.dataset.pickDestination;
    onDestinationChanged();
  });
  $('wizardStepper').addEventListener('click', (e) => {
    const step = e.target.closest('[data-goto-step]');
    if (step) goToStep(Number(step.dataset.gotoStep));
  });
  document.querySelector('#addJobModal .wizard-body').addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && e.target.tagName === 'INPUT') {
      e.preventDefault();
      wizardNextStep();
    }
  });
}
