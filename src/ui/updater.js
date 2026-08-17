import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';
import { formatReleaseDate } from '../utils/format.js';

export function updateVersionDisplay({ version, isLatest, publishedAt, updateAvailable, updateVersion }) {
  const versionBox = document.getElementById('versionBox');
  const statusBadge = document.getElementById('versionStatusBadge');
  const versionDate = document.getElementById('versionDate');
  const updateBox = document.getElementById('updateBox');

  const displayVersion = version
    ? (version.startsWith('v') ? version : `v${version}`)
    : '';

  if (versionBox && displayVersion) versionBox.textContent = displayVersion;

  const sidebarVersion = document.getElementById('sidebarVersion');
  if (sidebarVersion && displayVersion) sidebarVersion.textContent = displayVersion;

  const settingsVersion = document.getElementById('settingsVersion');
  if (settingsVersion && displayVersion) settingsVersion.textContent = displayVersion;

  const settingsVersionBadge = document.getElementById('settingsVersionBadge');
  const settingsVersionDate = document.getElementById('settingsVersionDate');
  const settingsVersionHint = document.getElementById('settingsVersionHint');
  const settingsUpdateBtn = document.getElementById('settingsUpdateBtn');

  if (statusBadge) {
    if (updateAvailable) {
      statusBadge.textContent = 'Update';
      statusBadge.className = 'version-badge version-badge-update';
      statusBadge.style.display = '';
    } else {
      statusBadge.style.display = 'none';
    }
  }

  if (settingsVersionBadge) {
    if (updateAvailable) {
      settingsVersionBadge.textContent = 'Update';
      settingsVersionBadge.className = 'version-badge version-badge-update';
      settingsVersionBadge.style.display = '';
    } else {
      settingsVersionBadge.style.display = 'none';
    }
  }

  const dateLabel = publishedAt ? `on ${formatReleaseDate(publishedAt)}` : '';
  if (versionDate) versionDate.textContent = dateLabel;
  if (settingsVersionDate) settingsVersionDate.textContent = dateLabel;

  if (settingsVersionHint) {
    settingsVersionHint.textContent = updateAvailable
      ? `v${updateVersion || ''} is available`
      : isLatest
        ? 'You are on the latest release'
        : '';
  }

  if (updateBox) {
    if (updateAvailable) {
      updateBox.textContent = updateVersion ? `Install v${updateVersion}` : 'Install Update';
      updateBox.classList.add('show');
    } else {
      updateBox.classList.remove('show');
    }
  }

  if (settingsUpdateBtn) {
    if (updateAvailable) {
      settingsUpdateBtn.textContent = updateVersion ? `Install v${updateVersion}` : 'Install Update';
      settingsUpdateBtn.style.display = '';
    } else {
      settingsUpdateBtn.style.display = 'none';
    }
  }
}

export async function loadVersion() {
  try {
    const version = await invoke('get_app_version');
    updateVersionDisplay({ version, isLatest: true });
  } catch (e) {
    console.error('Failed to load version:', e);
  }
}

export async function checkForUpdates() {
  try {
    const currentVersion = await invoke('get_app_version');
    const result = await invoke('check_for_updates');

    updateVersionDisplay({
      version: currentVersion,
      isLatest: !result?.available,
      publishedAt: result?.published_at || '',
      updateAvailable: !!result?.available,
      updateVersion: result?.version || '',
    });
  } catch (e) {
    console.error('Failed to check for updates:', e);
    const updateBox = document.getElementById('updateBox');
    if (updateBox) updateBox.classList.remove('show');
  }
}

export async function handleUpdateClick() {
  const updateBox = document.getElementById('updateBox');
  if (!updateBox || !updateBox.classList.contains('show')) {
    return;
  }
  
  const confirmed = await confirm('Install update now? The application will restart after installation.', {
    title: 'Update Available',
    kind: 'info'
  });
  
  if (!confirmed) {
    return;
  }
  
  try {
    updateBox.textContent = 'Installing update...';
    updateBox.style.pointerEvents = 'none';
    
    await invoke('install_update');
    // App will restart automatically after installation
  } catch (e) {
    alert('Failed to install update: ' + e);
    updateBox.textContent = 'Update Available - Click to Install';
    updateBox.style.pointerEvents = 'auto';
  }
}
