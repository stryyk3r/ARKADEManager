import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { state } from '../../state.js';

export async function browsePluginSource() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Source Folder (containing plugin subdirectories)'
    });
    
    if (selected) {
      state.pluginSourcePath = selected;
      document.getElementById('pluginSourcePath').textContent = `Source: ${selected}`;
      await loadSourcePlugins(selected);
    }
  } catch (error) {
    console.error('Error browsing source folder:', error);
    alert('Failed to browse source folder: ' + error);
  }
};

async function loadSourcePlugins(sourcePath) {
  try {
    const plugins = await invoke('list_source_plugins', { sourcePath });
    state.pluginSourcePlugins = plugins;
    renderSourcePlugins();
    updateInstallButtonState();
  } catch (error) {
    console.error('Error loading source plugins:', error);
    document.getElementById('pluginSourceList').innerHTML = 
      `<div class="empty-state" style="color: var(--error);">Error: ${error}</div>`;
  }
}

function renderSourcePlugins() {
  const container = document.getElementById('pluginSourceList');
  const toggleBtn = document.getElementById('sourceToggleAllBtn');

  if (state.pluginSourcePlugins.length === 0) {
    container.innerHTML = '<div class="empty-state">No plugin folders found in source directory</div>';
    if (toggleBtn) toggleBtn.disabled = true;
    return;
  }

  container.innerHTML = state.pluginSourcePlugins.map((plugin, index) => `
    <div class="plugin-item">
      <input type="checkbox" id="source-plugin-${index}" data-path="${plugin.path}">
      <label for="source-plugin-${index}" class="plugin-item-label">${plugin.name}</label>
    </div>
  `).join('');

  container.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
    checkbox.addEventListener('change', () => {
      updateInstallButtonState();
      updatePluginToggleAllLabels();
    });
  });

  if (toggleBtn) toggleBtn.disabled = false;
  updatePluginToggleAllLabels();
}

export async function refreshPluginDestinations() {
  try {
    const destinations = await invoke('discover_plugin_destinations');
    state.pluginDestinations = destinations;
    renderDestinations();
    updateInstallButtonState();
  } catch (error) {
    console.error('Error loading destinations:', error);
    document.getElementById('pluginDestinationList').innerHTML =
      `<div class="empty-state" style="color: var(--error);">Error: ${error}</div>`;
    const toggleBtn = document.getElementById('destToggleAllBtn');
    if (toggleBtn) toggleBtn.disabled = true;
  }
};

function renderDestinations() {
  const container = document.getElementById('pluginDestinationList');
  const toggleBtn = document.getElementById('destToggleAllBtn');

  if (state.pluginDestinations.length === 0) {
    container.innerHTML = '<div class="empty-state">No ARK servers found. Set the ASA Server Root Directory in Settings (each server should be a subfolder).</div>';
    if (toggleBtn) toggleBtn.disabled = true;
    return;
  }

  container.innerHTML = state.pluginDestinations.map((server, index) => `
    <div class="plugin-item">
      <input type="checkbox" id="dest-server-${index}" data-path="${server.plugin_path}">
      <label for="dest-server-${index}" class="plugin-item-label">${server.name}</label>
    </div>
  `).join('');

  container.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
    checkbox.addEventListener('change', () => {
      updateInstallButtonState();
      updatePluginToggleAllLabels();
    });
  });

  if (toggleBtn) toggleBtn.disabled = false;
  updatePluginToggleAllLabels();
}

function updatePluginToggleAllLabels() {
  const sourceBtn = document.getElementById('sourceToggleAllBtn');
  const destBtn = document.getElementById('destToggleAllBtn');
  const sourceBoxes = document.querySelectorAll('#pluginSourceList input[type="checkbox"]');
  const destBoxes = document.querySelectorAll('#pluginDestinationList input[type="checkbox"]');

  if (sourceBtn && sourceBoxes.length > 0) {
    const allSourceChecked = [...sourceBoxes].every(cb => cb.checked);
    sourceBtn.textContent = allSourceChecked ? 'Deselect All' : 'Select All';
  }

  if (destBtn && destBoxes.length > 0) {
    const allDestChecked = [...destBoxes].every(cb => cb.checked);
    destBtn.textContent = allDestChecked ? 'Deselect All' : 'Select All';
  }
}

export function toggleAllSourcePlugins() {
  const checkboxes = document.querySelectorAll('#pluginSourceList input[type="checkbox"]');
  if (checkboxes.length === 0) return;
  const allChecked = [...checkboxes].every(cb => cb.checked);
  checkboxes.forEach(cb => { cb.checked = !allChecked; });
  updatePluginToggleAllLabels();
  updateInstallButtonState();
};

export function toggleAllDestinations() {
  const checkboxes = document.querySelectorAll('#pluginDestinationList input[type="checkbox"]');
  if (checkboxes.length === 0) return;
  const allChecked = [...checkboxes].every(cb => cb.checked);
  checkboxes.forEach(cb => { cb.checked = !allChecked; });
  updatePluginToggleAllLabels();
  updateInstallButtonState();
};

export function updateInstallButtonState() {
  const sourceSelected = document.querySelectorAll('#pluginSourceList input[type="checkbox"]:checked').length > 0;
  const destSelected = document.querySelectorAll('#pluginDestinationList input[type="checkbox"]:checked').length > 0;
  const installBtn = document.getElementById('installPluginsBtn');
  
    if (installBtn) {
    installBtn.disabled = !(sourceSelected && destSelected);
  }
}

export async function installSelectedPlugins() {
  const sourceCheckboxes = document.querySelectorAll('#pluginSourceList input[type="checkbox"]:checked');
  const destCheckboxes = document.querySelectorAll('#pluginDestinationList input[type="checkbox"]:checked');
  
  if (sourceCheckboxes.length === 0 || destCheckboxes.length === 0) {
    alert('Please select at least one source plugin and one destination server');
    return;
  }
  
  // Get selected plugin and server names for confirmation
  const selectedPlugins = Array.from(sourceCheckboxes).map(cb => {
    const label = document.querySelector(`label[for="${cb.id}"]`);
    return label ? label.textContent : 'Unknown';
  });
  const selectedServers = Array.from(destCheckboxes).map(cb => {
    const label = document.querySelector(`label[for="${cb.id}"]`);
    return label ? label.textContent : 'Unknown';
  });
  
  const sourcePaths = Array.from(sourceCheckboxes).map(cb => cb.dataset.path);
  const destPaths = Array.from(destCheckboxes).map(cb => cb.dataset.path);
  
  // Store installation data for confirmation
  state.pendingInstallation = {
    sourcePaths,
    destPaths,
    selectedPlugins,
    selectedServers,
    sourceCheckboxes: Array.from(sourceCheckboxes),
    destCheckboxes: Array.from(destCheckboxes)
  };
  
  // Show confirmation modal
  showPluginConfirmModal(selectedPlugins, selectedServers);
};

export async function proceedWithPluginInstallation() {
  if (!state.pendingInstallation) {
    return;
  }
  
  // Copy data before closing modal (closePluginConfirmModal sets state.pendingInstallation = null)
  const { sourcePaths, destPaths, selectedPlugins, selectedServers, sourceCheckboxes, destCheckboxes } = state.pendingInstallation;
  state.pendingInstallation = null;
  const modal = document.getElementById('pluginConfirmModal');
  if (modal) modal.classList.remove('show');
  
  const installBtn = document.getElementById('installPluginsBtn');
  installBtn.disabled = true;
  installBtn.textContent = 'Installing...';
  
  try {
    const result = await invoke('install_plugins', {
      sourcePluginPaths: sourcePaths,
      destinationPluginPaths: destPaths
    });

    showPluginResults(result, selectedPlugins, selectedServers);
    
    // Clear selections
    sourceCheckboxes.forEach(cb => cb.checked = false);
    destCheckboxes.forEach(cb => cb.checked = false);
    updateInstallButtonState();
    
  } catch (error) {
    console.error('Error installing plugins:', error);
    showPluginResultsError(error);
  } finally {
    if (installBtn) {
      installBtn.disabled = false;
      installBtn.textContent = 'Install Selected Plugins';
    }
  }
};

function showPluginConfirmModal(selectedPlugins, selectedServers) {
  const modal = document.getElementById('pluginConfirmModal');
  const content = document.getElementById('pluginConfirmContent');
  
  let html = '<div style="margin-bottom: 16px;">';
  html += `<p style="margin-bottom: 12px; font-size: 15px;">Are you sure you want to install the following plugins to the selected servers?</p>`;
  html += '</div>';
  
  html += '<div style="margin-bottom: 16px; padding: 12px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 4px;">';
  html += `<strong>Plugins to install (${selectedPlugins.length}):</strong><br>`;
  html += `<div style="margin-top: 8px; margin-left: 12px;">${selectedPlugins.join(', ')}</div>`;
  html += '</div>';
  
  html += '<div style="margin-bottom: 16px; padding: 12px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 4px;">';
  html += `<strong>Destination servers (${selectedServers.length}):</strong><br>`;
  html += `<div style="margin-top: 8px; margin-left: 12px;">${selectedServers.join(', ')}</div>`;
  html += '</div>';
  
  html += '<div style="padding: 12px; background: rgba(255, 193, 7, 0.1); border: 1px solid #ffc107; border-radius: 4px; color: #f57c00;">';
  html += '<strong>⚠ Note:</strong> Files with the same name will be overwritten. Other files in the destination folder are left unchanged.';
  html += '</div>';
  
  content.innerHTML = html;
  modal.classList.add('show');
}

export function closePluginConfirmModal() {
  const modal = document.getElementById('pluginConfirmModal');
  modal.classList.remove('show');
  state.pendingInstallation = null;
};

function showPluginResults(result, selectedPlugins, selectedServers) {
  const modal = document.getElementById('pluginResultsModal');
  const title = document.getElementById('pluginResultsTitle');
  const content = document.getElementById('pluginResultsContent');

  if (!modal || !title || !content) {
    console.error('Plugin results modal is missing from the DOM');
    return;
  }

  const confirmModal = document.getElementById('pluginConfirmModal');
  if (confirmModal) {
    confirmModal.classList.remove('show');
  }

  title.textContent = 'Installation Complete';

  let html = '<div style="margin-bottom: 16px;">';
  html += `<strong>Plugins installed:</strong> ${selectedPlugins.join(', ')}<br>`;
  html += `<strong>Servers updated:</strong> ${selectedServers.join(', ')}<br><br>`;
  html += '</div>';

  html += '<div style="margin-bottom: 16px;">';
  html += `<strong>Files copied:</strong> ${result.files_copied}<br>`;
  html += `<strong>Files overwritten:</strong> ${result.files_overwritten}`;
  html += '</div>';

  if (result.errors && result.errors.length > 0) {
    html += '<div style="margin-top: 16px; padding: 12px; background: rgba(211, 47, 47, 0.1); border: 1px solid var(--error); border-radius: 4px;">';
    html += '<strong style="color: var(--error);">Errors:</strong><ul style="margin-top: 8px; padding-left: 20px;">';
    result.errors.forEach(error => {
      html += `<li style="color: var(--error); margin-bottom: 4px;">${error}</li>`;
    });
    html += '</ul></div>';
  } else {
    html += '<div style="margin-top: 16px; padding: 12px; background: rgba(56, 142, 60, 0.1); border: 1px solid var(--success); border-radius: 4px; color: var(--success);">';
    html += '<strong>Installation completed successfully.</strong>';
    html += '</div>';
  }

  content.innerHTML = html;
  modal.classList.add('show');
}

function showPluginResultsError(error) {
  const modal = document.getElementById('pluginResultsModal');
  const title = document.getElementById('pluginResultsTitle');
  const content = document.getElementById('pluginResultsContent');

  if (!modal || !title || !content) {
    alert('Installation failed: ' + error);
    return;
  }

  const confirmModal = document.getElementById('pluginConfirmModal');
  if (confirmModal) {
    confirmModal.classList.remove('show');
  }

  title.textContent = 'Installation Failed';
  content.innerHTML = `
    <div style="padding: 12px; background: rgba(211, 47, 47, 0.1); border: 1px solid var(--error); border-radius: 4px; color: var(--error);">
      <strong>Error:</strong> ${error}
    </div>
  `;
  modal.classList.add('show');
}

export function closePluginResultsModal() {
  const modal = document.getElementById('pluginResultsModal');
  if (!modal) return;
  modal.classList.remove('show');
}
