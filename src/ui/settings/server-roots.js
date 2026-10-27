import { invoke } from '@tauri-apps/api/core';
import { state, MONTHLY_CLUSTERS } from '../../state.js';
import { escapeHtml, debounce, setMessage, setButtonBusy } from '../../utils/dom.js';
import { plural } from '../../utils/format.js';
import { pickDirectory } from '../jobs/form.js';

const $ = (id) => document.getElementById(id);

const ROOTS = [
  { key: 'asa_server_root', input: 'asaServerRootInput', browse: 'browseAsaServerRootBtn', check: 'asaServerRootCheck' },
  { key: 'minecraft_server_root', input: 'minecraftServerRootInput', browse: 'browseMinecraftServerRootBtn', check: 'minecraftServerRootCheck' },
  { key: 'palworld_server_root', input: 'palworldServerRootInput', browse: 'browsePalworldServerRootBtn', check: 'palworldServerRootCheck' },
];

async function checkRoot(root) {
  const input = $(root.input);
  const check = $(root.check);
  const path = input.value.trim() || input.placeholder;
  try {
    const info = await invoke('inspect_directory', { path });
    if ((input.value.trim() || input.placeholder) !== path) return;
    const found = info.exists;
    check.textContent = found ? `${plural(info.child_dirs.length, 'server')} found` : 'Folder not found';
    check.className = `dir-check ${found ? 'ok' : 'warn'}`;
    input.classList.toggle('is-warning', !found && !!input.value.trim());
  } catch {
    check.textContent = '';
  }
}

/** Called after config changes elsewhere (e.g. save) so other pages pick up the new roots. */
let onRootsSaved = () => {};

export function setupServerRootSettings({ onSaved } = {}) {
  if (onSaved) onRootsSaved = onSaved;
  const config = state.config || {};
  ROOTS.forEach(root => {
    const input = $(root.input);
    input.value = config[root.key] || '';
    const recheck = debounce(() => checkRoot(root), 300);
    input.addEventListener('input', () => {
      setMessage($('serverRootsSaveMessage'), '');
      recheck();
    });
    $(root.browse).addEventListener('click', async () => {
      const selected = await pickDirectory('Select server root directory', input.value.trim());
      if (selected) {
        input.value = selected;
        checkRoot(root);
      }
    });
    checkRoot(root);
  });

  $('saveServerRootsBtn').addEventListener('click', async () => {
    const btn = $('saveServerRootsBtn');
    setButtonBusy(btn, true, 'Saving…');
    try {
      state.config = await invoke('save_server_roots', {
        asaServerRoot: $('asaServerRootInput').value.trim() || null,
        minecraftServerRoot: $('minecraftServerRootInput').value.trim() || null,
        palworldServerRoot: $('palworldServerRootInput').value.trim() || null,
      });
      setMessage($('serverRootsSaveMessage'), 'Server directories saved.', 'success');
      onRootsSaved();
    } catch (e) {
      setMessage($('serverRootsSaveMessage'), String(e), 'error');
    } finally {
      setButtonBusy(btn, false);
    }
  });
}

export function setupMonthlySettings(effectiveDestination) {
  const input = $('monthlyDestinationInput');
  input.value = state.config?.monthly_archive_destination || effectiveDestination || '';
  $('monthlyClusterTags').innerHTML = MONTHLY_CLUSTERS.map(c => `<span class="tag">${escapeHtml(c)}</span>`).join('');

  let saved = input.value.trim();
  const save = async () => {
    const value = input.value.trim();
    if (value === saved) return;
    try {
      state.config = await invoke('save_monthly_destination', { path: value || null });
      saved = value;
      setMessage($('monthlySaveMessage'), value ? 'Monthly archive folder saved.' : 'Using the default monthly folder.', 'success');
    } catch (e) {
      setMessage($('monthlySaveMessage'), String(e), 'error');
    }
  };
  input.addEventListener('change', save);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') save();
  });
  $('browseMonthlyDestinationBtn').addEventListener('click', async () => {
    const selected = await pickDirectory('Select monthly archive folder', input.value.trim());
    if (selected) {
      input.value = selected;
      save();
    }
  });
}
