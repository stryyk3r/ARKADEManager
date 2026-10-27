import { invoke } from '@tauri-apps/api/core';
import { state } from '../../state.js';
import { escapeHtml, onSegmentedChange, setSegmented, setButtonBusy } from '../../utils/dom.js';
import { basename, shortenPath, plural } from '../../utils/format.js';
import { confirmDialog, listHtml, toast } from '../feedback.js';
import { clusterForServer, pluginsDirFor } from './servers.js';

const $ = (id) => document.getElementById(id);

function selectedServer() {
  return state.pluginServers.find(s => s.root === state.pluginToggle.server) || null;
}

function isOn(folder) {
  const pending = state.pluginToggle.pending;
  return pending.has(folder.base_name) ? pending.get(folder.base_name) : !folder.is_disabled;
}

async function loadFolders(server) {
  try {
    server.folders = await invoke('list_plugin_folders', { serverRoot: server.root });
    server.error = null;
  } catch (e) {
    server.folders = null;
    server.error = String(e);
  }
}

export async function loadPluginToggleServers() {
  try {
    const roots = await invoke('get_plugin_server_roots');
    const previous = new Map(state.pluginServers.map(s => [s.root, s]));
    state.pluginServers = roots.map(root => previous.get(root) || { root, name: basename(root), folders: null, error: null });
    state.pluginServers.forEach(s => { s.cluster = clusterForServer(s.root); });
    await Promise.all(state.pluginServers.map(loadFolders));
  } catch (e) {
    state.pluginServers = [];
    $('pluginServerList').innerHTML = `<div class="empty compact is-error">${escapeHtml(String(e))}</div>`;
    return;
  }
  if (!selectedServer()) {
    state.pluginToggle.server = state.pluginServers[0]?.root ?? null;
    state.pluginToggle.pending.clear();
  }
  renderServerList();
  renderFolders();
}

function renderServerList() {
  const list = $('pluginServerList');
  if (state.pluginServers.length === 0) {
    list.innerHTML = '<div class="empty compact">No ASA servers found. Set the ASA server directory in Settings.</div>';
    return;
  }
  const needle = state.pluginToggle.search.trim().toLowerCase();
  const servers = state.pluginServers.filter(s => !needle || `${s.name} ${s.cluster}`.toLowerCase().includes(needle));
  if (servers.length === 0) {
    list.innerHTML = '<div class="empty compact">No servers match.</div>';
    return;
  }
  list.innerHTML = servers.map(s => {
    const selected = s.root === state.pluginToggle.server;
    const count = s.folders ? `${s.folders.filter(f => !f.is_disabled).length} / ${s.folders.length}` : '—';
    return `<button class="server-row ${selected ? 'selected' : ''}" type="button" data-server="${escapeHtml(s.root)}" aria-pressed="${selected}" title="${escapeHtml(s.root)}">
      <span class="text"><span class="name" style="display:block">${escapeHtml(s.name)}</span><span class="cluster" style="display:block">${escapeHtml(s.cluster || (s.error ? 'No plugins folder' : 'No backup job'))}</span></span>
      <span class="count" title="enabled / installed">${count}</span>
    </button>`;
  }).join('');
}

function renderFolders() {
  const server = selectedServer();
  const list = $('pluginToggleFoldersList');
  $('pluginFolderHead').hidden = !server;
  if (!server) {
    list.innerHTML = '<div class="empty compact">Select a server to see its plugin folders.</div>';
    renderPendingBar();
    return;
  }
  $('pluginFolderServerName').textContent = server.name;
  const dir = pluginsDirFor(server.root);
  $('pluginFolderPath').textContent = shortenPath(dir, 6);
  $('pluginFolderPath').title = dir;

  const folders = server.folders || [];
  const onCount = folders.filter(isOn).length;
  const counts = { all: folders.length, on: onCount, off: folders.length - onCount };
  document.querySelectorAll('#pluginFolderFilter [data-count]').forEach(el => { el.textContent = counts[el.dataset.count]; });

  if (server.error) {
    list.innerHTML = `<div class="empty compact is-error">${escapeHtml(server.error)}</div>`;
  } else if (folders.length === 0) {
    list.innerHTML = '<div class="empty compact">No plugin folders on this server.</div>';
  } else {
    const filter = state.pluginToggle.filter;
    const shown = folders.filter(f => filter === 'all' || (filter === 'on') === isOn(f));
    list.innerHTML = shown.length === 0
      ? `<div class="empty compact">No plugins are ${filter}.</div>`
      : shown.map(f => {
        const on = isOn(f);
        const pending = state.pluginToggle.pending.has(f.base_name);
        return `<div class="folder-row ${on ? '' : 'off'} ${pending ? 'is-pending' : ''}" data-folder="${escapeHtml(f.base_name)}">
          <button class="switch" type="button" role="switch" aria-checked="${on}" aria-label="${escapeHtml(f.base_name)}"></button>
          <span class="name">${escapeHtml(f.base_name)}</span>
          <span class="dir" title="${escapeHtml(f.full_path)}">${escapeHtml(f.name)}</span>
          <span class="pending">${pending ? (on ? 'turning on' : 'turning off') : ''}</span>
        </div>`;
      }).join('');
  }
  renderPendingBar();
}

function renderPendingBar() {
  const server = selectedServer();
  const pending = [...state.pluginToggle.pending.entries()];
  const has = pending.length > 0;
  $('pluginToggleBar').classList.toggle('is-idle', !has);
  $('pluginPendingSummary').textContent = has ? plural(pending.length, 'pending change') : 'No pending changes';
  $('pluginPendingDetail').textContent = has
    ? pending.map(([name, on]) => `${name} ${on ? 'on' : 'off'}`).join(', ')
    : 'Flip a switch, then apply. Restart the server to load the change.';
  $('discardPluginChangesBtn').disabled = !has;
  const allBtn = $('toggleAllServersBtn');
  const curBtn = $('toggleCurrentServerBtn');
  if (allBtn.dataset.label === undefined) {
    allBtn.disabled = !has;
    allBtn.textContent = `Apply to all ${state.pluginServers.length} servers`;
  }
  if (curBtn.dataset.label === undefined) {
    curBtn.disabled = !has;
    curBtn.textContent = server ? `Apply to ${server.name}` : 'Apply';
  }
}

async function selectServer(root) {
  if (root === state.pluginToggle.server) return;
  if (state.pluginToggle.pending.size > 0) {
    const discard = await confirmDialog({
      title: 'Discard pending changes?',
      body: `<p>${plural(state.pluginToggle.pending.size, 'change')} on ${escapeHtml(selectedServer()?.name || 'this server')} have not been applied.</p>`,
      confirmLabel: 'Discard',
      danger: true,
    });
    if (!discard) return;
  }
  state.pluginToggle.server = root;
  state.pluginToggle.pending.clear();
  renderServerList();
  renderFolders();
}

function toggleFolder(baseName) {
  const server = selectedServer();
  const folder = server?.folders?.find(f => f.base_name === baseName);
  if (!folder) return;
  const target = !isOn(folder);
  if (target === !folder.is_disabled) state.pluginToggle.pending.delete(baseName);
  else state.pluginToggle.pending.set(baseName, target);
  renderFolders();
  $('pluginToggleFoldersList').querySelector(`[data-folder="${CSS.escape(baseName)}"] .switch`)?.focus({ preventScroll: true });
}

export function discardPluginChanges() {
  state.pluginToggle.pending.clear();
  renderFolders();
}

function reportErrors(errors, successMessage) {
  if (errors.length) {
    toast(`${plural(errors.length, 'folder')} could not be renamed`, { kind: 'error', detail: errors.join('\n') });
  } else {
    toast(successMessage, { kind: 'success', detail: 'Restart the server to load the change.' });
  }
}

export async function togglePluginsForCurrentServer() {
  const server = selectedServer();
  const pending = [...state.pluginToggle.pending.entries()];
  if (!server || pending.length === 0) return;
  const btn = $('toggleCurrentServerBtn');
  setButtonBusy(btn, true, 'Applying…');
  const errors = [];
  try {
    for (const [baseName, targetOn] of pending) {
      const folder = server.folders?.find(f => f.base_name === baseName);
      if (!folder || targetOn === !folder.is_disabled) continue;
      try {
        await invoke('toggle_plugin_folder', { folderPath: folder.full_path });
      } catch (e) {
        errors.push(`${baseName}: ${e}`);
      }
    }
  } finally {
    setButtonBusy(btn, false);
  }
  state.pluginToggle.pending.clear();
  await loadFolders(server);
  renderServerList();
  renderFolders();
  reportErrors(errors, `Applied ${plural(pending.length, 'change')} to ${server.name}`);
}

export async function togglePluginsForAllServers() {
  const pending = [...state.pluginToggle.pending.entries()];
  if (pending.length === 0) return;
  const confirmed = await confirmDialog({
    title: `Apply to all ${state.pluginServers.length} servers?`,
    body: `<p>Every server that has these plugins is set to match:</p>${listHtml(pending.map(([n, on]) => `${n} → ${on ? 'on' : 'off'}`))}`,
    confirmLabel: 'Apply to all',
  });
  if (!confirmed) return;

  const btn = $('toggleAllServersBtn');
  setButtonBusy(btn, true, 'Applying…');
  const errors = [];
  let renamed = 0;
  try {
    for (const [baseName, targetOn] of pending) {
      try {
        const paths = await invoke('toggle_plugin_for_all_servers', { baseFolderName: baseName, targetStateDisabled: !targetOn });
        renamed += paths.length;
      } catch (e) {
        errors.push(`${baseName}: ${e}`);
      }
    }
  } finally {
    setButtonBusy(btn, false);
  }
  state.pluginToggle.pending.clear();
  await Promise.all(state.pluginServers.map(loadFolders));
  renderServerList();
  renderFolders();
  reportErrors(errors, `Renamed ${plural(renamed, 'folder')} across all servers`);
}

export function setupPluginToggle() {
  $('pluginServerSearch').addEventListener('input', (e) => {
    state.pluginToggle.search = e.target.value;
    renderServerList();
  });
  $('pluginServerList').addEventListener('click', (e) => {
    const row = e.target.closest('[data-server]');
    if (row) selectServer(row.dataset.server);
  });
  $('pluginToggleFoldersList').addEventListener('click', (e) => {
    const row = e.target.closest('[data-folder]');
    if (row) toggleFolder(row.dataset.folder);
  });
  onSegmentedChange($('pluginFolderFilter'), (value) => {
    state.pluginToggle.filter = value;
    renderFolders();
  });
  setSegmented($('pluginFolderFilter'), state.pluginToggle.filter);
  $('discardPluginChangesBtn').addEventListener('click', discardPluginChanges);
}
