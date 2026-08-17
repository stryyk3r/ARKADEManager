import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { loadPluginToggleServers } from '../plugins/toggle.js';
import { refreshPluginDestinations } from '../plugins/install.js';

function showServerRootsMessage(text, type = 'info') {
  const el = document.getElementById('serverRootsSaveMessage');
  if (!el) return;
  el.hidden = !text;
  el.textContent = text || '';
  el.classList.remove('is-error', 'is-success');
  if (type === 'error') el.classList.add('is-error');
  if (type === 'success') el.classList.add('is-success');
}

async function loadServerRootSettings() {
  try {
    const config = await invoke('get_config');
    const asaInput = document.getElementById('asaServerRootInput');
    const minecraftInput = document.getElementById('minecraftServerRootInput');
    const palworldInput = document.getElementById('palworldServerRootInput');
    if (asaInput) asaInput.value = config.asa_server_root || '';
    if (minecraftInput) minecraftInput.value = config.minecraft_server_root || '';
    if (palworldInput) palworldInput.value = config.palworld_server_root || '';
    showServerRootsMessage('');
  } catch (e) {
    console.error('Failed to load server root settings:', e);
    showServerRootsMessage('Failed to load server directories.', 'error');
  }
}

async function pickServerRootDirectory(inputId) {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select server root directory',
    });
    if (selected) {
      const input = document.getElementById(inputId);
      if (input) input.value = selected;
    }
  } catch (e) {
    console.error('Failed to pick directory:', e);
    alert('Failed to open directory picker: ' + e.message);
  }
}

export function setupServerRootSettings() {
  loadServerRootSettings();

  document.getElementById('browseAsaServerRootBtn')?.addEventListener('click', () => {
    pickServerRootDirectory('asaServerRootInput');
  });
  document.getElementById('browseMinecraftServerRootBtn')?.addEventListener('click', () => {
    pickServerRootDirectory('minecraftServerRootInput');
  });
  document.getElementById('browsePalworldServerRootBtn')?.addEventListener('click', () => {
    pickServerRootDirectory('palworldServerRootInput');
  });

  document.getElementById('saveServerRootsBtn')?.addEventListener('click', async () => {
    try {
      await invoke('save_server_roots', {
        asaServerRoot: document.getElementById('asaServerRootInput')?.value.trim() || null,
        minecraftServerRoot: document.getElementById('minecraftServerRootInput')?.value.trim() || null,
        palworldServerRoot: document.getElementById('palworldServerRootInput')?.value.trim() || null,
      });
      showServerRootsMessage('Server directories saved.', 'success');
      await loadPluginToggleServers();
      await refreshPluginDestinations();
    } catch (e) {
      console.error('Failed to save server roots:', e);
      showServerRootsMessage(String(e), 'error');
    }
  });
}
