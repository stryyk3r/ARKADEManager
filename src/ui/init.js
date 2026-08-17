import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { state } from '../state.js';
import { GITHUB_RELEASES_URL } from '../state.js';
import { setupDataLookupForm } from './data-lookup.js';
import { applyJobFilters } from './jobs/filters.js';
import { refreshJobs, updateStatus, updateStatusFromEvent, showBackupProgress, hideBackupProgress } from './jobs/table.js';
import { closeAddJobModal } from './wizard/backup-wizard.js';
import { refreshLogs } from './logs.js';
import { loadVersion, checkForUpdates, handleUpdateClick } from './updater.js';
import { refreshMapSelects, setupArkMapsSettings } from './settings/maps.js';
import { setupServerRootSettings } from './settings/server-roots.js';
import { loadPluginToggleServers, loadPluginFolders, initAllAdminSelects } from './plugins/toggle.js';
import { initWizardBackupTypeListeners } from './wizard/backup-wizard.js';
import { refreshPluginDestinations } from './plugins/install.js';
import { closePluginResultsModal, closePluginConfirmModal } from './plugins/install.js';

function updateThemeIcon(theme) {
  const icon = document.getElementById('themeToggleIcon');
  if (icon) icon.textContent = theme === 'dark' ? '☀' : '☾';
}

export function switchTab(tabName) {
  document.querySelectorAll('.nav-item').forEach(t => {
    t.classList.remove('active');
  });
  document.querySelectorAll('.tab-pane').forEach(p => p.classList.remove('active'));

  const activeTab = document.querySelector(`.nav-item[data-tab="${tabName}"]`);
  if (activeTab) activeTab.classList.add('active');
  document.getElementById(tabName).classList.add('active');

  if (tabName === 'new-plugins') {
    refreshPluginDestinations();
  }

  if (tabName === 'plugin-toggle') {
    loadPluginToggleServers();
  }
}

function setupPluginModals() {
  const resultsModal = document.getElementById('pluginResultsModal');
  if (resultsModal) {
    resultsModal.addEventListener('click', (e) => {
      if (e.target === resultsModal) {
        closePluginResultsModal();
      }
    });
  }

  const confirmModal = document.getElementById('pluginConfirmModal');
  if (confirmModal) {
    confirmModal.addEventListener('click', (e) => {
      if (e.target === confirmModal) {
        closePluginConfirmModal();
      }
    });
  }
}

export async function initApp() {
  try {
    const config = await invoke('get_config');
    const theme = config.theme || 'dark';
    document.documentElement.setAttribute('data-theme', theme);
    updateThemeIcon(theme);
  } catch (e) {
    document.documentElement.setAttribute('data-theme', 'dark');
    updateThemeIcon('dark');
  }

  document.querySelectorAll('.nav-item').forEach(tab => {
    tab.addEventListener('click', () => {
      const tabName = tab.dataset.tab;
      switchTab(tabName);
    });
  });

  setupDataLookupForm();

  document.getElementById('themeToggle').addEventListener('click', async () => {
    const currentTheme = document.documentElement.getAttribute('data-theme') || 'dark';
    const newTheme = currentTheme === 'light' ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', newTheme);
    updateThemeIcon(newTheme);
    try {
      await invoke('set_theme', { theme: newTheme });
    } catch (e) {
      console.error('Failed to save theme:', e);
    }
  });

  const jobSearchInput = document.getElementById('jobSearchInput');
  const jobStatusFilter = document.getElementById('jobStatusFilter');
  const jobTypeFilter = document.getElementById('jobTypeFilter');
  if (jobSearchInput) jobSearchInput.addEventListener('input', applyJobFilters);
  if (jobStatusFilter) jobStatusFilter.addEventListener('change', applyJobFilters);
  if (jobTypeFilter) jobTypeFilter.addEventListener('change', applyJobFilters);

  await refreshJobs();

  state.jobsRefreshInterval = setInterval(refreshJobs, 30000);
  state.statusUpdateInterval = setInterval(updateStatus, 2000);
  state.logsRefreshInterval = setInterval(refreshLogs, 5000);

  await listen('status_update', (event) => {
    updateStatusFromEvent(event.payload);
  });

  await listen('job_updated', () => {
    hideBackupProgress();
    refreshJobs();
  });

  await listen('backup_progress', (event) => {
    const p = event.payload || {};
    const jobName = p.job_name || 'Backup';
    const percent = Math.min(100, Math.max(0, Number(p.percent) || 0));
    showBackupProgress(jobName, percent);
  });

  await listen('backup_failed', (event) => {
    hideBackupProgress();
    const p = event.payload || {};
    const name = p.job_name || 'Backup';
    const err = p.error || 'Unknown error';
    const lower = String(err).toLowerCase();
    const isWarning = lower.includes('completed with warnings') || lower.includes('warning');
    const header = isWarning ? 'Backup warning' : 'Backup failed';
    alert(`${header}: ${name}\n\n${err}\n\nThe backup will not run again until the next scheduled time.`);
  });

  await updateStatus();
  await refreshLogs();

  await loadVersion();

  await checkForUpdates();
  state.updateCheckInterval = setInterval(checkForUpdates, 3600000);

  const updateBox = document.getElementById('updateBox');
  if (updateBox) {
    updateBox.addEventListener('click', handleUpdateClick);
  }
  const settingsUpdateBtn = document.getElementById('settingsUpdateBtn');
  if (settingsUpdateBtn) {
    settingsUpdateBtn.addEventListener('click', handleUpdateClick);
  }

  const openReleaseNotes = () => {
    invoke('open_external_url', { url: GITHUB_RELEASES_URL }).catch(err => {
      console.error('Failed to open release notes:', err);
    });
  };
  document.getElementById('settingsReleaseNotesBtn')?.addEventListener('click', openReleaseNotes);

  await refreshMapSelects(false);
  setupArkMapsSettings();
  setupServerRootSettings();

  await loadPluginToggleServers();
  initAllAdminSelects();
  initWizardBackupTypeListeners();
  setupPluginModals();

  const pluginToggleServerSelect = document.getElementById('pluginToggleServerSelect');
  if (pluginToggleServerSelect) {
    pluginToggleServerSelect.addEventListener('change', async (e) => {
      const serverRoot = e.target.value;
      const container = document.getElementById('pluginToggleFoldersContainer');

      if (serverRoot) {
        container.style.display = 'block';
        await loadPluginFolders(serverRoot);
      } else {
        container.style.display = 'none';
      }
    });
  }

  const modal = document.getElementById('addJobModal');
  if (modal) {
    modal.addEventListener('click', (e) => {
      if (e.target === modal) {
        closeAddJobModal();
      }
    });
  }
}
