import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';
import { state } from '../../state.js';

export function initAdminSelect(selectEl) {
  if (!selectEl || selectEl.dataset.adminSelectWired === 'true') return;
  selectEl.dataset.adminSelectWired = 'true';
  selectEl.classList.add('admin-select-native');

  const wrap = document.createElement('div');
  wrap.className = 'admin-select';
  selectEl.parentNode.insertBefore(wrap, selectEl);
  wrap.appendChild(selectEl);

  const trigger = document.createElement('button');
  trigger.type = 'button';
  trigger.className = 'admin-select-trigger';
  trigger.setAttribute('aria-haspopup', 'listbox');
  trigger.setAttribute('aria-expanded', 'false');
  trigger.innerHTML = `
    <span class="admin-select-value"></span>
    <svg class="admin-select-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
      <path d="m6 9 6 6 6-6"/>
    </svg>`;
  wrap.appendChild(trigger);

  const menu = document.createElement('div');
  menu.className = 'admin-select-menu';
  menu.setAttribute('role', 'listbox');
  menu.hidden = true;
  wrap.appendChild(menu);

  const valueEl = trigger.querySelector('.admin-select-value');

  function closeMenu() {
    menu.hidden = true;
    trigger.setAttribute('aria-expanded', 'false');
  }

  function syncFromSelect() {
    const opt = selectEl.options[selectEl.selectedIndex];
    valueEl.textContent = opt ? opt.textContent : '';
    valueEl.classList.toggle('is-placeholder', !selectEl.value);
    menu.querySelectorAll('.admin-select-item').forEach(item => {
      item.classList.toggle('is-selected', item.dataset.value === selectEl.value);
    });
  }

  function rebuildMenu() {
    menu.innerHTML = '';
    [...selectEl.options].forEach(opt => {
      const item = document.createElement('button');
      item.type = 'button';
      item.className = 'admin-select-item';
      item.dataset.value = opt.value;
      item.setAttribute('role', 'option');
      item.textContent = opt.textContent;
      item.addEventListener('click', (e) => {
        e.stopPropagation();
        selectEl.value = opt.value;
        selectEl.dispatchEvent(new Event('change', { bubbles: true }));
        closeMenu();
        syncFromSelect();
      });
      menu.appendChild(item);
    });
    syncFromSelect();
  }

  trigger.addEventListener('click', (e) => {
    e.stopPropagation();
    const willOpen = menu.hidden;
    document.querySelectorAll('.admin-select-menu').forEach(m => { m.hidden = true; });
    document.querySelectorAll('.admin-select-trigger').forEach(t => t.setAttribute('aria-expanded', 'false'));
    if (willOpen) {
      menu.hidden = false;
      trigger.setAttribute('aria-expanded', 'true');
    }
  });

  if (!window._adminSelectCloseListener) {
    document.addEventListener('click', () => {
      document.querySelectorAll('.admin-select-menu').forEach(m => { m.hidden = true; });
      document.querySelectorAll('.admin-select-trigger').forEach(t => t.setAttribute('aria-expanded', 'false'));
    });
    window._adminSelectCloseListener = true;
  }

  selectEl.addEventListener('change', syncFromSelect);
  selectEl._syncAdminSelect = syncFromSelect;
  new MutationObserver(rebuildMenu).observe(selectEl, { childList: true });
  rebuildMenu();
}

export function initAllAdminSelects(root = document) {
  root.querySelectorAll('select').forEach(selectEl => {
    if (selectEl.dataset.adminSelectWired !== 'true') {
      initAdminSelect(selectEl);
    }
  });
}

export async function loadPluginToggleServers() {
  try {
    const servers = await invoke('get_plugin_server_roots');
    const select = document.getElementById('pluginToggleServerSelect');
    if (!select) return;

    const previous = select.value;
    select.innerHTML = '<option value="">Select a server</option>';

    if (servers.length === 0) {
      const option = document.createElement('option');
      option.value = '';
      option.textContent = 'No servers found — set ASA root in Settings';
      option.disabled = true;
      select.appendChild(option);
    } else {
      servers.forEach(server => {
        const option = document.createElement('option');
        option.value = server;
        const parts = server.split(/[/\\]/);
        option.textContent = parts[parts.length - 1] || server;
        option.title = server;
        select.appendChild(option);
      });
      if (previous && [...select.options].some(opt => opt.value === previous)) {
        select.value = previous;
      }
    }

    if (select.dataset.adminSelectWired !== 'true') {
      initAdminSelect(select);
    }

    select.dispatchEvent(new Event('change', { bubbles: true }));
  } catch (e) {
    console.error('Failed to load servers:', e);
  }
}

export async function loadPluginFolders(serverRoot) {
  try {
    const folders = await invoke('list_plugin_folders', { serverRoot });
    const container = document.getElementById('pluginToggleFoldersList');
    container.innerHTML = '';
    
    if (folders.length === 0) {
      container.innerHTML = '<div class="empty-state">No plugin folders found</div>';
      return;
    }
    
    folders.forEach(folder => {
      const item = document.createElement('div');
      item.className = `plugin-folder-item ${folder.is_disabled ? 'disabled' : ''}`;
      
      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.value = folder.full_path;
      checkbox.dataset.baseName = folder.base_name;
      checkbox.addEventListener('change', updateToggleButtons);
      
      const label = document.createElement('label');
      label.className = 'folder-name';
      label.textContent = folder.name;
      label.style.cursor = 'pointer';
      label.style.margin = 0;
      label.style.flex = 1;
      
      label.addEventListener('click', (e) => {
        e.preventDefault();
        checkbox.checked = !checkbox.checked;
        checkbox.dispatchEvent(new Event('change'));
      });
      
      item.appendChild(checkbox);
      item.appendChild(label);
      container.appendChild(item);
    });
    
    state.selectedPluginFolders.clear();
    updateToggleButtons();
  } catch (e) {
    console.error('Failed to load plugin folders:', e);
    const container = document.getElementById('pluginToggleFoldersList');
    container.innerHTML = `<div class="empty-state" style="color: var(--error-color);">Error: ${e}</div>`;
  }
}

export function updateToggleButtons() {
  const checkboxes = document.querySelectorAll('#pluginToggleFoldersList input[type="checkbox"]:checked');
  const hasSelection = checkboxes.length > 0;
  
  document.getElementById('toggleCurrentServerBtn').disabled = !hasSelection;
  document.getElementById('toggleAllServersBtn').disabled = !hasSelection;
}

export async function togglePluginsForCurrentServer() {
  const checkboxes = document.querySelectorAll('#pluginToggleFoldersList input[type="checkbox"]:checked');

  if (checkboxes.length === 0) {
    alert('Please select at least one folder to toggle');
    return;
  }

  try {
    const confirmed = await confirm(`Toggle ${checkboxes.length} folder(s) for the current server?`, {
      title: 'Toggle Plugins',
      kind: 'info'
    });

    if (!confirmed) {
      return;
    }

    const serverRoot = document.getElementById('pluginToggleServerSelect').value;
    const errors = [];

    for (const checkbox of checkboxes) {
      try {
        await invoke('toggle_plugin_folder', { folderPath: checkbox.value });
      } catch (e) {
        console.error('Error toggling folder:', checkbox.value, e);
        errors.push(`${checkbox.dataset.baseName}: ${e}`);
      }
    }

    if (errors.length > 0) {
      alert('Some folders failed to toggle:\n' + errors.join('\n'));
    }

    await loadPluginFolders(serverRoot);
  } catch (e) {
    console.error('Error in togglePluginsForCurrentServer:', e);
    alert('Failed to toggle plugins: ' + e);
  }
}

export async function togglePluginsForAllServers() {
  const checkboxes = document.querySelectorAll('#pluginToggleFoldersList input[type="checkbox"]:checked');

  if (checkboxes.length === 0) {
    alert('Please select at least one folder to toggle');
    return;
  }

  const serverRoot = document.getElementById('pluginToggleServerSelect').value;
  if (!serverRoot) {
    alert('Please select a server first');
    return;
  }

  const folders = await invoke('list_plugin_folders', { serverRoot });
  const folderStates = new Map();
  folders.forEach(folder => {
    folderStates.set(folder.base_name, folder.is_disabled);
  });

  const foldersToToggle = [];
  for (const checkbox of checkboxes) {
    const baseName = checkbox.dataset.baseName;
    const currentStateDisabled = folderStates.get(baseName) || false;
    const targetStateDisabled = !currentStateDisabled;

    foldersToToggle.push({
      baseName,
      targetStateDisabled,
      currentState: currentStateDisabled ? 'disabled' : 'enabled',
      targetState: targetStateDisabled ? 'disabled' : 'enabled'
    });
  }

  const uniqueFolders = [];
  const seen = new Set();
  for (const folder of foldersToToggle) {
    if (!seen.has(folder.baseName)) {
      seen.add(folder.baseName);
      uniqueFolders.push(folder);
    }
  }

  const folderList = uniqueFolders.map(f =>
    `${f.baseName} (${f.currentState} → ${f.targetState})`
  ).join('\n');

  try {
    const confirmed = await confirm(
      `Set ${uniqueFolders.length} folder(s) across all servers to match current server state?\n\n` +
      `Folders:\n${folderList}`,
      {
        title: 'Toggle Plugins for All Servers',
        kind: 'info'
      }
    );

    if (!confirmed) {
      return;
    }

    const errors = [];
    const toggledPaths = [];

    for (const folder of uniqueFolders) {
      try {
        const paths = await invoke('toggle_plugin_for_all_servers', {
          baseFolderName: folder.baseName,
          targetStateDisabled: folder.targetStateDisabled
        });
        toggledPaths.push(...paths);
      } catch (e) {
        console.error('Error toggling folder:', folder.baseName, e);
        errors.push(`${folder.baseName}: ${e}`);
      }
    }

    if (errors.length > 0) {
      alert('Some folders failed to toggle:\n' + errors.join('\n'));
    } else {
      alert(`Successfully set ${toggledPaths.length} folder(s) across all servers`);
    }

    if (serverRoot) {
      await loadPluginFolders(serverRoot);
    }
  } catch (e) {
    console.error('Error in togglePluginsForAllServers:', e);
    alert('Failed to toggle plugins: ' + e);
  }
}
