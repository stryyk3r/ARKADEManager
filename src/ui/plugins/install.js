import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { state, MONTHLY_CLUSTERS } from '../../state.js';
import { escapeHtml, checkHtml, setCheck, setButtonBusy, storageGet, storageSet } from '../../utils/dom.js';
import { plural } from '../../utils/format.js';
import { openOverlay, closeOverlay, confirmDialog, listHtml, toastError } from '../feedback.js';
import { clusterForServer } from './servers.js';

const SOURCE_PATH_KEY = 'arkade.pluginSourcePath';
const KEEP_CONFIG_KEY = 'arkade.keepPluginConfig';
const OTHER_GROUP = 'Other servers';

const $ = (id) => document.getElementById(id);

let sourceError = null;

// ---------- Source ----------

export async function browsePluginSource() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select the folder that holds your plugin folders',
      defaultPath: state.pluginSourcePath || undefined,
    });
    if (typeof selected === 'string') {
      state.pluginInstall.sourceSel.clear();
      await loadSourcePlugins(selected);
    }
  } catch (error) {
    toastError('Could not open the folder picker', error);
  }
}

async function loadSourcePlugins(sourcePath) {
  state.pluginSourcePath = sourcePath;
  $('pluginSourcePath').value = sourcePath;
  $('pluginSourcePath').title = sourcePath;
  try {
    state.pluginSourcePlugins = await invoke('list_source_plugins', { sourcePath });
    storageSet(SOURCE_PATH_KEY, sourcePath);
    const known = new Set(state.pluginSourcePlugins.map(p => p.path));
    state.pluginInstall.sourceSel.forEach(p => { if (!known.has(p)) state.pluginInstall.sourceSel.delete(p); });
    sourceError = null;
  } catch (error) {
    state.pluginSourcePlugins = [];
    state.pluginInstall.sourceSel.clear();
    sourceError = String(error);
  }
  renderInstallState();
}

function renderSourcePlugins() {
  const error = sourceError;
  const list = $('pluginSourceList');
  const plugins = state.pluginSourcePlugins;
  $('sourceSelectAll').hidden = plugins.length === 0;

  if (error) {
    list.innerHTML = `<div class="empty compact is-error">${escapeHtml(error)}</div>`;
    return;
  }
  if (!state.pluginSourcePath) {
    list.innerHTML = '<div class="empty compact">Choose the folder that holds your plugin folders.</div>';
    return;
  }
  if (plugins.length === 0) {
    list.innerHTML = '<div class="empty compact">No plugin folders in this folder.</div>';
    return;
  }
  list.innerHTML = plugins.map(p => {
    const on = state.pluginInstall.sourceSel.has(p.path);
    return `<button class="pick-row source-row ${on ? 'selected' : ''}" type="button" data-source="${escapeHtml(p.path)}" aria-pressed="${on}">
      ${checkHtml(on)}
      <span class="name">${escapeHtml(p.name)}</span>
      <span class="version">${escapeHtml(p.version || '')}</span>
    </button>`;
  }).join('');
}

// ---------- Destinations ----------

export async function refreshPluginDestinations() {
  try {
    state.pluginDestinations = await invoke('discover_plugin_destinations');
    const known = new Set(state.pluginDestinations.map(d => d.plugin_path));
    state.pluginInstall.destSel.forEach(p => { if (!known.has(p)) state.pluginInstall.destSel.delete(p); });
    renderInstallState();
  } catch (error) {
    $('pluginDestinationList').innerHTML = `<div class="empty compact is-error">${escapeHtml(String(error))}</div>`;
  }
}

function destinationGroups() {
  const groups = new Map();
  for (const dest of state.pluginDestinations) {
    const cluster = clusterForServer(dest.path) || OTHER_GROUP;
    if (!groups.has(cluster)) groups.set(cluster, []);
    groups.get(cluster).push(dest);
  }
  const order = [...MONTHLY_CLUSTERS, ...[...groups.keys()].filter(k => !MONTHLY_CLUSTERS.includes(k) && k !== OTHER_GROUP).sort(), OTHER_GROUP];
  return order.filter(k => groups.has(k)).map(name => ({ name, servers: groups.get(name) }));
}

function selectedSources() {
  return state.pluginSourcePlugins.filter(p => state.pluginInstall.sourceSel.has(p.path));
}

function selectedDestinations() {
  return state.pluginDestinations.filter(d => state.pluginInstall.destSel.has(d.plugin_path));
}

/** kind: 'new' | 'update' | 'same' | 'off' (installed but disabled: installs a second, enabled copy) */
function classify(source, dest) {
  const installed = (dest.installed || []).find(p => p.name.toLowerCase() === source.name.toLowerCase());
  if (!installed) return { kind: 'new', label: 'new' };
  if (installed.disabled) return { kind: 'off', label: 'new copy · _OFF kept' };
  if (source.version && installed.version) {
    return source.version === installed.version
      ? { kind: 'same', label: 'same' }
      : { kind: 'update', label: `${installed.version} → ${source.version}` };
  }
  return { kind: 'update', label: 'replace' };
}

function impactFor(dest, sources) {
  if (sources.length === 0) return { text: '', cls: '' };
  const counts = { new: 0, update: 0, same: 0, off: 0 };
  sources.forEach(s => { counts[classify(s, dest).kind] += 1; });
  const parts = [];
  if (counts.new) parts.push(`${counts.new} new`);
  if (counts.update) parts.push(plural(counts.update, 'update'));
  if (!parts.length && counts.same) parts.push('up to date');
  if (counts.off) parts.push(`${counts.off} beside _OFF`);
  return { text: parts.join(' · '), cls: counts.new ? 'new' : counts.update ? 'updates' : '' };
}

function renderDestinations() {
  const list = $('pluginDestinationList');
  if (state.pluginDestinations.length === 0) {
    list.innerHTML = '<div class="empty compact">No ASA servers found. Set the ASA server directory in Settings.</div>';
    return;
  }
  const sources = selectedSources();
  list.innerHTML = destinationGroups().map(group => {
    const allOn = group.servers.every(s => state.pluginInstall.destSel.has(s.plugin_path));
    const rows = group.servers.map(dest => {
      const on = state.pluginInstall.destSel.has(dest.plugin_path);
      const meta = dest.plugins_dir_exists ? plural((dest.installed || []).length, 'plugin') + ' installed' : 'No ArkApi plugins folder';
      const impact = on ? impactFor(dest, sources) : { text: '', cls: '' };
      return `<button class="pick-row dest-row ${on ? 'selected' : ''}" type="button" data-dest="${escapeHtml(dest.plugin_path)}" aria-pressed="${on}" title="${escapeHtml(dest.path)}">
        ${checkHtml(on)}
        <span class="text"><span class="name" style="display:block">${escapeHtml(dest.name)}</span><span class="meta" style="display:block">${escapeHtml(meta)}</span></span>
        <span class="impact ${impact.cls}">${escapeHtml(impact.text)}</span>
      </button>`;
    }).join('');
    return `<div class="subhead"><span>${escapeHtml(group.name)}</span>
      <button class="link-btn" type="button" data-select-group="${escapeHtml(group.name)}">${allOn ? 'Clear' : 'Select all'}</button></div>${rows}`;
  }).join('');
}

// ---------- Review ----------

function reviewItems() {
  const items = [];
  for (const source of selectedSources()) {
    for (const dest of selectedDestinations()) {
      items.push({ source, dest, ...classify(source, dest) });
    }
  }
  return items;
}

function renderReview() {
  const items = reviewItems();
  const installs = items;
  const counts = { new: 0, update: 0, same: 0, off: 0 };
  items.forEach(i => { counts[i.kind] += 1; });
  const sources = selectedSources();
  const dests = selectedDestinations();

  $('reviewTotal').textContent = plural(installs.length, 'install');
  $('reviewSub').textContent = sources.length && dests.length
    ? `${plural(sources.length, 'plugin')} on ${plural(dests.length, 'server')}`
    : !sources.length ? 'Pick plugins to install' : 'Pick destination servers';
  $('reviewLegend').innerHTML = items.length ? [
    `<span><span class="dot new"></span>${counts.new} new</span>`,
    `<span><span class="dot update"></span>${plural(counts.update, 'update')}</span>`,
    `<span><span class="dot same"></span>${counts.same} same</span>`,
    counts.off ? `<span title="Disabled (_OFF) on that server. Installing adds an enabled copy next to it."><span class="dot off"></span>${counts.off} beside _OFF</span>` : '',
  ].join('') : '';

  $('reviewList').innerHTML = items.map(i => `
    <div class="review-item">
      <span class="what" title="${escapeHtml(`${i.source.name} → ${i.dest.name}`)}"><strong>${escapeHtml(i.source.name)}</strong> <span>→ ${escapeHtml(i.dest.name)}</span></span>
      <span class="status ${i.kind}">${escapeHtml(i.label)}</span>
    </div>`).join('');

  const btn = $('installPluginsBtn');
  if (btn.dataset.label === undefined) {
    btn.disabled = installs.length === 0;
    btn.textContent = installs.length ? `Install ${installs.length}` : 'Install';
  }
}

function focusKey() {
  const el = document.activeElement;
  if (el?.dataset?.source) return `[data-source="${CSS.escape(el.dataset.source)}"]`;
  if (el?.dataset?.dest) return `[data-dest="${CSS.escape(el.dataset.dest)}"]`;
  if (el?.dataset?.selectGroup) return `[data-select-group="${CSS.escape(el.dataset.selectGroup)}"]`;
  return null;
}

function renderInstallState() {
  const refocus = focusKey();
  const sel = state.pluginInstall.sourceSel.size;
  const total = state.pluginSourcePlugins.length;
  $('sourceSelectedCount').textContent = total ? `${sel} of ${total} selected` : '';
  setCheck($('sourceSelectAll').querySelector('.check'), sel === 0 ? false : sel === total ? true : 'mixed');
  $('destSelectedCount').textContent = state.pluginDestinations.length
    ? `${state.pluginInstall.destSel.size} of ${state.pluginDestinations.length} selected`
    : '';
  renderSourcePlugins();
  renderDestinations();
  renderReview();
  if (refocus) document.querySelector(`#pluginInstallPanel ${refocus}`)?.focus({ preventScroll: true });
}

// ---------- Install ----------

function offWarningHtml(items) {
  const off = items.filter(i => i.kind === 'off');
  if (off.length === 0) return '';
  return `<p class="text-yellow"><strong>${off.length === 1 ? '1 install goes' : `${off.length} installs go`} next to a disabled copy.</strong>
    These servers already have the plugin turned off (_OFF). Installing adds a second, enabled folder,
    which loads on the next restart alongside the _OFF one, which stays as it is.</p>
    ${listHtml(off.map(i => `${i.source.name}_OFF on ${i.dest.name}`))}`;
}

export async function installSelectedPlugins() {
  const items = reviewItems();
  if (items.length === 0) return;
  const keepConfig = $('keepPluginConfig').checked;
  const sources = [...new Set(items.map(i => i.source.name))];
  const servers = [...new Set(items.map(i => i.dest.name))];

  const confirmed = await confirmDialog({
    title: `Install ${plural(items.length, 'plugin copy', 'plugin copies')}?`,
    body: `<p><strong>${escapeHtml(sources.join(', '))}</strong> on ${plural(servers.length, 'server')}.</p>
      <p>Files with the same name are replaced; other files are left alone.${keepConfig ? ' Existing config.json files are kept.' : ' <strong>config.json files are overwritten.</strong>'}</p>
      ${offWarningHtml(items)}
      <p class="note">A running server may lock its plugin DLLs. Those files are left unchanged and listed as errors.</p>`,
    confirmLabel: `Install ${items.length}`,
  });
  if (!confirmed) return;

  const btn = $('installPluginsBtn');
  setButtonBusy(btn, true, 'Installing…');
  const totals = { files_copied: 0, files_overwritten: 0, files_skipped: 0, errors: [] };
  try {
    // One call per server, so each server's results and errors stay separate.
    const byDest = new Map();
    items.forEach(i => {
      if (!byDest.has(i.dest.plugin_path)) byDest.set(i.dest.plugin_path, []);
      byDest.get(i.dest.plugin_path).push(i.source.path);
    });
    for (const [destPath, sourcePaths] of byDest) {
      try {
        const result = await invoke('install_plugins', {
          sourcePluginPaths: sourcePaths,
          destinationPluginPaths: [destPath],
          keepExistingConfig: keepConfig,
        });
        totals.files_copied += result.files_copied;
        totals.files_overwritten += result.files_overwritten;
        totals.files_skipped += result.files_skipped || 0;
        totals.errors.push(...(result.errors || []));
      } catch (error) {
        totals.errors.push(`${destPath}: ${error}`);
      }
    }
    showPluginResults(totals, sources, servers);
    state.pluginInstall.destSel.clear();
  } finally {
    setButtonBusy(btn, false);
    await refreshPluginDestinations();
  }
}

function showPluginResults(result, plugins, servers) {
  const failed = result.errors.length > 0;
  $('pluginResultsTitle').textContent = failed ? 'Installed with errors' : 'Installation complete';
  $('pluginResultsSub').textContent = `${plugins.join(', ')} → ${plural(servers.length, 'server')}`;
  $('pluginResultsContent').innerHTML = `
    <div class="result-stats">
      <span><strong>${result.files_copied}</strong><span>new files</span></span>
      <span><strong>${result.files_overwritten}</strong><span>replaced</span></span>
      ${result.files_skipped ? `<span><strong>${result.files_skipped}</strong><span>configs kept</span></span>` : ''}
    </div>
    ${failed
      ? `<div class="result-errors">${result.errors.map(e => escapeHtml(String(e))).join('\n')}</div>`
      : '<div class="result-ok">Every file was copied.</div>'}`;
  openOverlay($('pluginResultsModal'), { onDismiss: closePluginResultsModal });
}

export function closePluginResultsModal() {
  closeOverlay($('pluginResultsModal'));
}

// ---------- Setup ----------

export function setupPluginInstall() {
  $('keepPluginConfig').checked = storageGet(KEEP_CONFIG_KEY, true);
  $('keepPluginConfig').addEventListener('change', (e) => storageSet(KEEP_CONFIG_KEY, e.target.checked));

  $('pluginSourceList').addEventListener('click', (e) => {
    const row = e.target.closest('[data-source]');
    if (!row) return;
    const path = row.dataset.source;
    const sel = state.pluginInstall.sourceSel;
    if (sel.has(path)) sel.delete(path); else sel.add(path);
    renderInstallState();
  });

  $('sourceSelectAll').addEventListener('click', () => {
    const sel = state.pluginInstall.sourceSel;
    const all = state.pluginSourcePlugins.length > 0 && sel.size === state.pluginSourcePlugins.length;
    sel.clear();
    if (!all) state.pluginSourcePlugins.forEach(p => sel.add(p.path));
    renderInstallState();
  });

  $('pluginDestinationList').addEventListener('click', (e) => {
    const groupBtn = e.target.closest('[data-select-group]');
    const sel = state.pluginInstall.destSel;
    if (groupBtn) {
      const group = destinationGroups().find(g => g.name === groupBtn.dataset.selectGroup);
      if (!group) return;
      const allOn = group.servers.every(s => sel.has(s.plugin_path));
      group.servers.forEach(s => (allOn ? sel.delete(s.plugin_path) : sel.add(s.plugin_path)));
      renderInstallState();
      return;
    }
    const row = e.target.closest('[data-dest]');
    if (!row) return;
    const path = row.dataset.dest;
    if (sel.has(path)) sel.delete(path); else sel.add(path);
    renderInstallState();
  });

  const savedSource = storageGet(SOURCE_PATH_KEY, null);
  if (savedSource) loadSourcePlugins(savedSource);
  else renderInstallState();
}
