import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { state, GITHUB_RELEASES_URL, MONTHLY_CLUSTERS, INTERVAL_UNITS } from '../state.js';
import { fillSelect, onSegmentedChange, setSegmented } from '../utils/dom.js';
import { initAllSelects } from './select.js';
import { toastError } from './feedback.js';
import { setupDataLookupForm, refreshDataLookupMeta } from './data-lookup.js';
import { setupJobFilters } from './jobs/filters.js';
import { setupEditDrawer } from './jobs/form.js';
import {
  refreshJobs, updateStatus, updateStatusFromEvent, showBackupProgress, hideBackupProgress,
  bindJobsTableEvents, loadCollapsedGroups,
} from './jobs/table.js';
import { initWizard } from './wizard/backup-wizard.js';
import { refreshLogs, setupLogs, showLogsTab } from './logs.js';
import { loadVersion, checkForUpdates, setupUpdates } from './updater.js';
import { refreshMapSelects, setupArkMapsSettings } from './settings/maps.js';
import { setupServerRootSettings, setupMonthlySettings } from './settings/server-roots.js';
import { setupPluginsPage, showPluginsPage } from './plugins/page.js';

const TAB_ALIASES = { 'new-plugins': 'plugins', 'plugin-toggle': 'plugins' };

export function switchTab(tabName) {
  const tab = TAB_ALIASES[tabName] || tabName;
  if (!document.getElementById(tab)) return;
  document.querySelectorAll('.nav-item').forEach(t => {
    const on = t.dataset.tab === tab;
    t.classList.toggle('active', on);
    if (on) t.setAttribute('aria-current', 'page');
    else t.removeAttribute('aria-current');
  });
  document.querySelectorAll('.page').forEach(p => p.classList.toggle('active', p.id === tab));
  if (tab === 'plugins') showPluginsPage();
  if (tab === 'logs') showLogsTab();
}

function applyTheme(theme) {
  document.documentElement.setAttribute('data-theme', theme);
  setSegmented(document.getElementById('themeToggle'), theme);
}

function setupSettingsNav() {
  const page = document.getElementById('settings');
  const links = [...document.querySelectorAll('#settingsNav a')];
  const sections = links.map(a => document.querySelector(a.getAttribute('href')));
  const setActive = (index) => links.forEach((a, i) => a.classList.toggle('active', i === index));
  links.forEach((a, i) => a.addEventListener('click', (e) => {
    e.preventDefault();
    sections[i]?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setActive(i);
  }));
  page.addEventListener('scroll', () => {
    const top = page.getBoundingClientRect().top + 40;
    let active = 0;
    sections.forEach((s, i) => { if (s && s.getBoundingClientRect().top <= top) active = i; });
    if (page.scrollTop + page.clientHeight >= page.scrollHeight - 4) active = sections.length - 1;
    setActive(active);
  }, { passive: true });
}

function fillStaticSelects() {
  const clusters = MONTHLY_CLUSTERS.map(c => ({ value: c, label: c }));
  document.querySelectorAll('select[data-clusters]').forEach(s => fillSelect(s, clusters));
  document.querySelectorAll('select[data-units]').forEach(s => fillSelect(s, INTERVAL_UNITS, 'minutes'));
}

export async function initApp() {
  let paths = null;
  try {
    state.config = await invoke('get_config');
  } catch (e) {
    state.config = {};
  }
  try {
    paths = await invoke('get_app_paths');
  } catch (e) {
    console.error('Failed to read app paths:', e);
  }
  applyTheme(state.config.theme === 'light' ? 'light' : 'dark');
  if (paths?.config_path) {
    const el = document.getElementById('settingsConfigPath');
    el.textContent = paths.config_path;
    el.title = paths.config_path;
  }

  fillStaticSelects();
  initAllSelects();
  loadCollapsedGroups();

  document.querySelectorAll('.nav-item').forEach(tab => {
    tab.addEventListener('click', () => switchTab(tab.dataset.tab));
  });

  onSegmentedChange(document.getElementById('themeToggle'), async (theme) => {
    applyTheme(theme);
    try {
      await invoke('set_theme', { theme });
    } catch (e) {
      toastError('Could not save the theme', e);
    }
  });

  bindJobsTableEvents();
  setupJobFilters();
  setupEditDrawer();
  initWizard();
  setupLogs(paths?.logs_dir);
  setupPluginsPage();
  setupDataLookupForm();
  setupSettingsNav();
  setupArkMapsSettings();
  setupServerRootSettings({
    onSaved: () => {
      refreshDataLookupMeta();
      if (document.getElementById('plugins').classList.contains('active')) showPluginsPage();
    },
  });
  setupMonthlySettings(paths?.monthly_destination);
  setupUpdates(() => {
    invoke('open_external_url', { url: GITHUB_RELEASES_URL }).catch(err => toastError('Could not open release notes', err));
  });

  await refreshMapSelects(false);
  await refreshJobs();

  state.jobsRefreshInterval = setInterval(refreshJobs, 30000);
  state.statusUpdateInterval = setInterval(updateStatus, 2000);
  state.logsRefreshInterval = setInterval(refreshLogs, 5000);

  await listen('status_update', (event) => updateStatusFromEvent(event.payload));
  await listen('job_updated', () => {
    hideBackupProgress();
    refreshJobs();
  });
  await listen('backup_progress', (event) => {
    const p = event.payload || {};
    const percent = Math.round(Math.min(100, Math.max(0, Number(p.percent) || 0)));
    showBackupProgress(p.job_name || 'Backup', percent);
  });
  await listen('backup_failed', () => {
    hideBackupProgress();
    refreshJobs();
  });

  await updateStatus();
  await loadVersion();
  checkForUpdates();
  state.updateCheckInterval = setInterval(checkForUpdates, 3600000);
}
