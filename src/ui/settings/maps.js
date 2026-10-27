import { invoke } from '@tauri-apps/api/core';
import { state } from '../../state.js';
import { populateMapSelect, escapeHtml, setMessage, setButtonBusy } from '../../utils/dom.js';
import { confirmDialog } from '../feedback.js';

const $ = (id) => document.getElementById(id);

const REMOVE_ICON = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg>';

/** Load maps into state and every map <select> (keeps current selections by default). */
export async function refreshMapSelects(preserveValues = true) {
  try {
    state.arkMaps = await invoke('get_ark_maps');
    for (const id of ['drawerMap', 'wizardMapSelect']) {
      const el = $(id);
      populateMapSelect(el, state.arkMaps, preserveValues ? el?.value : '');
    }
    return state.arkMaps;
  } catch (e) {
    console.error('Failed to load ARK maps:', e);
    return [];
  }
}

function showArkMapsMessage(text, type = 'info') {
  setMessage($('arkMapsSaveMessage'), text, type);
}

function renderArkMapsSettingsList(maps) {
  state.editableArkMaps = maps.map(map => ({ ...map }));
  const list = $('arkMapsSettingsList');
  if (state.editableArkMaps.length === 0) {
    list.innerHTML = '<div class="empty compact">No maps. Add one or reset to the defaults.</div>';
    return;
  }
  const input = (field, index, value, cls, placeholder) =>
    `<input type="text" class="${cls}" data-field="${field}" data-index="${index}" value="${escapeHtml(value)}" placeholder="${placeholder}" spellcheck="false" aria-label="${placeholder}">`;
  list.innerHTML = state.editableArkMaps.map((map, i) => `
    <div class="maps-row maps-grid">
      ${input('display_name', i, map.display_name, '', 'Display name')}
      ${input('id', i, map.id, 'mono', 'ID')}
      ${input('folder_name', i, map.folder_name, 'mono dim', 'Saves folder')}
      ${input('map_file_name', i, map.map_file_name, 'mono dim', 'Map file')}
      <button type="button" class="icon-btn" data-remove-index="${i}" aria-label="Remove ${escapeHtml(map.display_name || 'map')}">${REMOVE_ICON}</button>
    </div>`).join('');
}

async function loadArkMapsSettings() {
  try {
    renderArkMapsSettingsList(await invoke('get_ark_maps'));
    showArkMapsMessage('');
  } catch (e) {
    showArkMapsMessage('Failed to load maps.', 'error');
  }
}

export function setupArkMapsSettings() {
  const list = $('arkMapsSettingsList');
  list.addEventListener('input', (e) => {
    const el = e.target;
    if (!(el instanceof HTMLInputElement) || !el.dataset.field) return;
    const map = state.editableArkMaps[Number(el.dataset.index)];
    if (map) map[el.dataset.field] = el.value;
  });

  list.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-remove-index]');
    if (!btn) return;
    state.editableArkMaps.splice(Number(btn.dataset.removeIndex), 1);
    renderArkMapsSettingsList(state.editableArkMaps);
    showArkMapsMessage('Map removed. Save to keep the change.', 'warning');
  });

  $('addArkMapBtn').addEventListener('click', () => {
    state.editableArkMaps.push({ id: '', display_name: '', folder_name: '', map_file_name: '' });
    renderArkMapsSettingsList(state.editableArkMaps);
    list.querySelector('.maps-row:last-child input')?.focus();
  });

  $('resetArkMapsBtn').addEventListener('click', async () => {
    const confirmed = await confirmDialog({
      title: 'Reset ARK maps?',
      body: '<p>Replaces the list with the built-in maps. Custom maps you added are removed.</p>',
      confirmLabel: 'Reset maps',
      danger: true,
    });
    if (!confirmed) return;
    try {
      renderArkMapsSettingsList(await invoke('reset_ark_maps'));
      await refreshMapSelects(true);
      showArkMapsMessage('Restored the default maps.', 'success');
    } catch (e) {
      showArkMapsMessage(String(e), 'error');
    }
  });

  $('saveArkMapsBtn').addEventListener('click', async () => {
    const btn = $('saveArkMapsBtn');
    setButtonBusy(btn, true, 'Saving…');
    try {
      const maps = state.editableArkMaps.map(m => ({
        id: m.id.trim(),
        display_name: m.display_name.trim(),
        folder_name: m.folder_name.trim(),
        map_file_name: m.map_file_name.trim(),
      }));
      renderArkMapsSettingsList(await invoke('save_ark_maps', { maps }));
      await refreshMapSelects(true);
      showArkMapsMessage('Maps saved.', 'success');
    } catch (e) {
      showArkMapsMessage(String(e), 'error');
    } finally {
      setButtonBusy(btn, false);
    }
  });

  loadArkMapsSettings();
}
