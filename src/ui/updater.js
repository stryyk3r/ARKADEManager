import { invoke } from '@tauri-apps/api/core';
import { escapeHtml, setButtonBusy } from '../utils/dom.js';
import { formatReleaseDate } from '../utils/format.js';
import { confirmDialog, toastError, toast } from './feedback.js';

const $ = (id) => document.getElementById(id);

let latest = { available: false, version: '' };

function withV(version) {
  return version ? (version.startsWith('v') ? version : `v${version}`) : '';
}

export function updateVersionDisplay({ version, publishedAt, updateAvailable, updateVersion, checked, error }) {
  const display = withV(version);
  if (display) {
    $('sidebarVersion').textContent = display;
    $('settingsVersion').textContent = display;
  }
  const hint = $('settingsVersionHint');
  if (error) {
    hint.innerHTML = '<span class="text-yellow">Could not check for updates</span>';
  } else if (!checked) {
    hint.textContent = 'Checking for updates…';
  } else if (updateAvailable) {
    hint.innerHTML = `<span class="avail">${escapeHtml(withV(updateVersion))} is available</span>${publishedAt ? ` · released ${escapeHtml(formatReleaseDate(publishedAt))}` : ''}`;
  } else {
    hint.innerHTML = `${publishedAt ? `released ${escapeHtml(formatReleaseDate(publishedAt))} · ` : ''}<span class="ok">Up to date</span>`;
  }
  const btn = $('settingsUpdateBtn');
  btn.hidden = !updateAvailable;
  if (updateAvailable && btn.dataset.label === undefined) btn.textContent = `Install ${withV(updateVersion)}`;
}

export async function loadVersion() {
  try {
    updateVersionDisplay({ version: await invoke('get_app_version'), checked: false });
  } catch (e) {
    console.error('Failed to load version:', e);
  }
}

export async function checkForUpdates({ manual = false } = {}) {
  const version = await invoke('get_app_version').catch(() => '');
  try {
    const result = await invoke('check_for_updates');
    latest = { available: !!result?.available, version: result?.version || '' };
    updateVersionDisplay({
      version,
      checked: true,
      publishedAt: result?.published_at || '',
      updateAvailable: latest.available,
      updateVersion: latest.version,
    });
    if (manual && !latest.available) toast('You are on the latest version', { kind: 'success' });
  } catch (e) {
    console.error('Failed to check for updates:', e);
    updateVersionDisplay({ version, checked: true, error: true });
    if (manual) toastError('Could not check for updates', e);
  }
}

export async function handleUpdateClick() {
  if (!latest.available) return;
  const confirmed = await confirmDialog({
    title: `Install ${withV(latest.version)}?`,
    body: '<p>The update downloads, then ARKADE Manager closes and restarts. Running backups must finish first.</p>',
    confirmLabel: 'Install and restart',
  });
  if (!confirmed) return;
  const btn = $('settingsUpdateBtn');
  setButtonBusy(btn, true, 'Installing…');
  try {
    await invoke('install_update');
  } catch (e) {
    toastError('Could not install the update', e);
    setButtonBusy(btn, false);
  }
}

export function setupUpdates(openReleaseNotes) {
  $('settingsUpdateBtn').addEventListener('click', handleUpdateClick);
  $('settingsReleaseNotesBtn').addEventListener('click', openReleaseNotes);
  $('settingsCheckUpdatesBtn').addEventListener('click', async () => {
    const btn = $('settingsCheckUpdatesBtn');
    setButtonBusy(btn, true, 'Checking…');
    await checkForUpdates({ manual: true });
    setButtonBusy(btn, false);
  });
}
