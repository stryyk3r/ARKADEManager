import { invoke } from '@tauri-apps/api/core';
import { state } from '../../state.js';
import { setSelectValue, populateMapSelect, escapeHtmlAttr } from '../../utils/dom.js';

export async function refreshMapSelects(preserveValues = true, explicitValues = {}) {
  try {
    const maps = await invoke('get_ark_maps');
    populateMapSelect(
      document.getElementById('mapSelect'),
      maps,
      explicitValues.mapSelect ?? (preserveValues ? document.getElementById('mapSelect')?.value : '')
    );
    populateMapSelect(
      document.getElementById('wizardMapSelect'),
      maps,
      explicitValues.wizardMapSelect ?? (preserveValues ? document.getElementById('wizardMapSelect')?.value : '')
    );
    return maps;
  } catch (e) {
    console.error('Failed to load ARK maps:', e);
    return [];
  }
}

function showArkMapsMessage(text, type = 'info') {
  const el = document.getElementById('arkMapsSaveMessage');
  if (!el) return;
  el.hidden = !text;
  el.textContent = text || '';
  el.classList.remove('is-error', 'is-success');
  if (type === 'error') el.classList.add('is-error');
  if (type === 'success') el.classList.add('is-success');
}

function renderArkMapsSettingsList(maps) {
  state.editableArkMaps = maps.map(map => ({ ...map }));
  const tbody = document.getElementById('arkMapsSettingsList');
  if (!tbody) return;

  tbody.innerHTML = '';
  state.editableArkMaps.forEach((map, index) => {
    const row = document.createElement('tr');
    row.innerHTML = `
      <td><input type="text" data-field="display_name" data-index="${index}" value="${escapeHtmlAttr(map.display_name)}"></td>
      <td><input type="text" data-field="id" data-index="${index}" value="${escapeHtmlAttr(map.id)}"></td>
      <td><input type="text" data-field="folder_name" data-index="${index}" value="${escapeHtmlAttr(map.folder_name)}"></td>
      <td><input type="text" data-field="map_file_name" data-index="${index}" value="${escapeHtmlAttr(map.map_file_name)}"></td>
      <td><button type="button" class="secondary btn-outline btn-sm maps-settings-remove" data-remove-index="${index}">Remove</button></td>
    `;
    tbody.appendChild(row);
  });
}


function collectArkMapsFromSettings() {
  const tbody = document.getElementById('arkMapsSettingsList');
  if (!tbody) return [];

  return [...tbody.querySelectorAll('tr')].map(row => {
    const get = (field) => row.querySelector(`input[data-field="${field}"]`)?.value.trim() || '';
    return {
      id: get('id'),
      display_name: get('display_name'),
      folder_name: get('folder_name'),
      map_file_name: get('map_file_name'),
    };
  });
}

async function loadArkMapsSettings() {
  try {
    const maps = await invoke('get_ark_maps');
    renderArkMapsSettingsList(maps);
    showArkMapsMessage('');
  } catch (e) {
    console.error('Failed to load ARK map settings:', e);
    showArkMapsMessage('Failed to load maps.', 'error');
  }
}

export function setupArkMapsSettings() {
  const tbody = document.getElementById('arkMapsSettingsList');
  tbody?.addEventListener('input', (e) => {
    const input = e.target;
    if (!(input instanceof HTMLInputElement) || !input.dataset.field) return;
    const index = Number(input.dataset.index);
    if (!Number.isInteger(index) || !state.editableArkMaps[index]) return;
    state.editableArkMaps[index][input.dataset.field] = input.value;
  });

  tbody?.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-remove-index]');
    if (!btn) return;
    const index = Number(btn.dataset.removeIndex);
    state.editableArkMaps.splice(index, 1);
    renderArkMapsSettingsList(state.editableArkMaps);
  });

  document.getElementById('addArkMapBtn')?.addEventListener('click', () => {
    state.editableArkMaps.push({
      id: '',
      display_name: '',
      folder_name: '',
      map_file_name: '',
    });
    renderArkMapsSettingsList(state.editableArkMaps);
  });

  document.getElementById('resetArkMapsBtn')?.addEventListener('click', async () => {
    try {
      const maps = await invoke('reset_ark_maps');
      renderArkMapsSettingsList(maps);
      await refreshMapSelects(true);
      showArkMapsMessage('Restored default map list.', 'success');
    } catch (e) {
      console.error('Failed to reset ARK maps:', e);
      showArkMapsMessage(String(e), 'error');
    }
  });

  document.getElementById('saveArkMapsBtn')?.addEventListener('click', async () => {
    try {
      const maps = collectArkMapsFromSettings();
      const saved = await invoke('save_ark_maps', { maps });
      renderArkMapsSettingsList(saved);
      await refreshMapSelects(true);
      showArkMapsMessage('Maps saved.', 'success');
    } catch (e) {
      console.error('Failed to save ARK maps:', e);
      showArkMapsMessage(String(e), 'error');
    }
  });

  loadArkMapsSettings();
}
