import { invoke } from '@tauri-apps/api/core';
import { state } from '../../state.js';
import { escapeHtml } from '../../utils/dom.js';
import { formatFileSize } from '../../utils/format.js';
import { applyJobFilters } from './filters.js';
import { runJobNow, openBackupLocation, updateJob, deleteJob } from './form.js';

const JOB_GROUP_ORDER = ["asa","ase","minecraft","palworld"];
const JOB_GROUP_META = {
  asa: { label: 'ARK: Survival Ascended', letter: 'A', color: 'teal' },
  ase: { label: 'ARK: Survival Evolved', letter: 'A', color: 'yellow' },
  minecraft: { label: 'Minecraft', letter: 'M', color: 'green' },
  palworld: { label: 'Palworld', letter: 'P', color: 'blue' },
};

export function showBackupProgress(jobName, percent) {
  const card = document.getElementById('backupProgressCard');
  const nameEl = document.getElementById('backupProgressJobName');
  const pctEl = document.getElementById('backupProgressPct');
  const barEl = document.getElementById('backupProgressBar');
  if (!card || !nameEl || !pctEl || !barEl) return;
  card.classList.add('show');
  nameEl.textContent = jobName;
  pctEl.textContent = percent + '%';
  barEl.style.width = percent + '%';
}

export function hideBackupProgress() {
  const card = document.getElementById('backupProgressCard');
  if (card) card.classList.remove('show');
}

export function getJobGroupKey(job) {
  if (job.job_type === 'minecraft') return 'minecraft';
  if (job.job_type === 'palworld') return 'palworld';
  const cluster = job.monthly_cluster || '';
  if (cluster === 'ASE Legacy') return 'ase';
  if (cluster === 'Palworld') return 'palworld';
  return 'asa';
}

export function computeSuccessRate(jobs) {
  const enabled = (jobs || []).filter(j => j.enabled);
  if (enabled.length === 0) return '—';
  const dayAgo = Date.now() - 24 * 60 * 60 * 1000;
  const ok = enabled.filter(j => {
    if (j.last_error) return false;
    if (!j.last_run_at) return false;
    return new Date(j.last_run_at).getTime() > dayAgo;
  }).length;
  return `${Math.round((ok / enabled.length) * 100)}%`;
}

export function computeTotalStorage(jobs) {
  const total = (jobs || []).reduce((sum, j) => sum + (j.last_file_size || 0), 0);
  return formatFileSize(total);
}

export function getJobRowState(job) {
  if (state.currentRunningJob && state.currentRunningJob === job.name) {
    return { dot: 'running', sub: 'in progress', running: true };
  }
  if (job.last_error) {
    const lower = String(job.last_error).toLowerCase();
    const isWarning = lower.includes('completed with warnings') || lower.includes('warning');
    return { dot: isWarning ? 'paused' : 'error', sub: isWarning ? 'warning' : 'error', running: false };
  }
  if (!job.enabled) {
    return { dot: 'paused', sub: 'paused', running: false };
  }
  return { dot: 'online', sub: '', running: false };
}

export function updateJobCounts(jobs) {
  const list = jobs || [];
  const configured = document.getElementById('configuredJobsBadge');
  if (configured) configured.textContent = `${list.length} configured`;

  const enabled = list.filter(j => j.enabled).length;
  const saving = state.currentRunningJob ? 1 : 0;

  const activeValue = document.getElementById('statActiveValue');
  const activeMeta = document.getElementById('statActiveMeta');
  if (activeValue) activeValue.textContent = String(saving);
  if (activeMeta) activeMeta.textContent = saving === 1 ? '1 saving' : `${saving} saving`;

  const successValue = document.getElementById('statSuccessValue');
  if (successValue) successValue.textContent = computeSuccessRate(list);

  const storageValue = document.getElementById('statStorageValue');
  if (storageValue) storageValue.textContent = computeTotalStorage(list);
}

export async function refreshJobs() {
  try {
    const jobs = await invoke('list_jobs');
    renderJobsTable(jobs);
  } catch (e) {
    console.error('Failed to refresh jobs:', e);
  }
}

export function renderJobsTable(jobs) {
  state.allJobs = jobs || [];
  const emptyAll = document.getElementById('backupEmptyAll');
  const toolbar = document.getElementById('backupToolbar');
  const jobsPanel = document.getElementById('jobsPanel');
  const tbody = document.getElementById('jobsTableBody');
  const hasAny = state.allJobs.length > 0;

  if (emptyAll) emptyAll.style.display = hasAny ? 'none' : 'block';
  if (toolbar) toolbar.style.display = hasAny ? '' : 'none';
  if (jobsPanel) jobsPanel.style.display = hasAny ? '' : 'none';

  updateJobCounts(state.allJobs);
  if (!tbody) return;
  tbody.innerHTML = '';

  const grouped = {};
  for (const job of state.allJobs) {
    const key = getJobGroupKey(job);
    if (!grouped[key]) grouped[key] = [];
    grouped[key].push(job);
  }

  for (const groupKey of JOB_GROUP_ORDER) {
    const groupJobs = grouped[groupKey];
    if (!groupJobs || groupJobs.length === 0) continue;
    const meta = JOB_GROUP_META[groupKey];
    const groupSize = groupJobs.reduce((sum, j) => sum + (j.last_file_size || 0), 0);

    const headerRow = tbody.insertRow();
    headerRow.className = 'group-header-row';
    headerRow.dataset.groupKey = groupKey;
    const headerCell = headerRow.insertCell();
    headerCell.colSpan = 6;
    headerCell.innerHTML = `
      <div class="group-header">
        <div class="group-header-left">
          <span class="group-accent group-accent-${meta.color}"></span>
          <span class="group-icon group-icon-${meta.color}">${meta.letter}</span>
          <span class="group-title">${escapeHtml(meta.label)}</span>
        </div>
        <div class="group-header-right">
          <span class="group-size">${escapeHtml(formatFileSize(groupSize))}</span>
          <span class="online-pill"><span class="status-dot"></span>Online</span>
        </div>
      </div>`;

    for (const job of groupJobs) {
      appendUnifiedJobRow(tbody, job, groupKey);
    }
  }

  applyJobFilters();

  ensureJobMenuListeners();
}

export function appendUnifiedJobRow(tbody, job, groupKey) {
  const row = tbody.insertRow();
  row.dataset.jobId = job.id;
  row.dataset.group = groupKey;
  const rowState = getJobRowState(job);

  const nameCell = row.insertCell();
  nameCell.innerHTML = `
    <div class="job-row-name">
      <span class="job-status-dot ${rowState.dot}"></span>
      <div class="job-name-block">
        <span class="job-name-primary ${rowState.running ? 'running' : ''}">${escapeHtml(job.name)}</span>
        ${rowState.sub ? `<span class="job-name-sub ${rowState.running ? 'progress' : ''}">${escapeHtml(rowState.sub)}</span>` : ''}
      </div>
    </div>`;

  const intervalCell = row.insertCell();
  intervalCell.textContent = `${job.interval_value} ${job.interval_unit}`;
  intervalCell.className = 'cell-metric cell-metric-muted';

  const nextCell = row.insertCell();
  nextCell.textContent = job.next_run_at ? new Date(job.next_run_at).toLocaleString() : 'N/A';

  const lastCell = row.insertCell();
  if (job.last_error && !rowState.running) {
    lastCell.innerHTML = rowState.sub === 'warning'
      ? '<span class="cell-metric cell-metric-warn">WARNING</span>'
      : '<span class="cell-metric cell-metric-bad">ERROR</span>';
    lastCell.title = String(job.last_error);
  } else {
    lastCell.textContent = job.last_run_at ? new Date(job.last_run_at).toLocaleString() : 'Never';
    lastCell.className = job.last_run_at ? 'cell-metric cell-metric-good' : 'cell-metric cell-metric-muted';
  }

  const sizeCell = row.insertCell();
  sizeCell.textContent = job.last_file_size ? formatFileSize(job.last_file_size) : 'N/A';
  sizeCell.className = 'cell-metric cell-metric-muted';

  const actionsCell = row.insertCell();
  actionsCell.className = 'cell-actions';
  const menuContainer = document.createElement('div');
  menuContainer.className = 'job-menu';
  const menuButton = document.createElement('button');
  menuButton.className = 'job-menu-button';
  menuButton.type = 'button';
  menuButton.textContent = '⋯';
  menuButton.onclick = (e) => {
    e.stopPropagation();
    toggleJobMenu(menuButton, dropdown);
  };
  const dropdown = document.createElement('div');
  dropdown.className = 'job-menu-dropdown';

  const runItem = document.createElement('button');
  runItem.className = 'job-menu-item';
  runItem.type = 'button';
  runItem.textContent = 'Run Now';
  runItem.onclick = (e) => { e.stopPropagation(); closeAllJobMenus(); runJobNow(job.id); };

  const backupLocationItem = document.createElement('button');
  backupLocationItem.className = 'job-menu-item';
  backupLocationItem.type = 'button';
  backupLocationItem.textContent = 'Backup Location';
  backupLocationItem.onclick = (e) => {
    e.stopPropagation();
    closeAllJobMenus();
    openBackupLocation(job.destination_dir);
  };

  const editItem = document.createElement('button');
  editItem.className = 'job-menu-item';
  editItem.type = 'button';
  editItem.textContent = 'Edit';
  editItem.onclick = (e) => {
    e.stopPropagation();
    closeAllJobMenus();
    updateJob(job.id);
  };

  const deleteItem = document.createElement('button');
  deleteItem.className = 'job-menu-item danger';
  deleteItem.type = 'button';
  deleteItem.textContent = 'Delete';
  deleteItem.onclick = (e) => {
    e.stopPropagation();
    closeAllJobMenus();
    deleteJob(job.id);
  };

  dropdown.appendChild(runItem);
  dropdown.appendChild(backupLocationItem);
  dropdown.appendChild(editItem);
  dropdown.appendChild(deleteItem);
  menuContainer.appendChild(menuButton);
  menuContainer.appendChild(dropdown);
  actionsCell.appendChild(menuContainer);
}

export function closeAllJobMenus() {
  document.querySelectorAll('.job-menu-dropdown').forEach(d => {
    d.classList.remove('show');
    d.style.top = '';
    d.style.left = '';
    d.style.visibility = '';
  });
}

export function positionJobMenuDropdown(button, dropdown) {
  dropdown.classList.add('show');
  dropdown.style.visibility = 'hidden';

  const rect = button.getBoundingClientRect();
  const gap = 4;
  const height = dropdown.offsetHeight;
  const width = dropdown.offsetWidth;

  const spaceBelow = window.innerHeight - rect.bottom - gap;
  const spaceAbove = rect.top - gap;
  const openBelow = spaceBelow >= height || spaceBelow >= spaceAbove;
  const top = openBelow ? rect.bottom + gap : rect.top - gap - height;

  let left = rect.right - width;
  left = Math.max(8, Math.min(left, window.innerWidth - width - 8));

  dropdown.style.top = `${Math.max(8, top)}px`;
  dropdown.style.left = `${left}px`;
  dropdown.style.visibility = '';
}

export function toggleJobMenu(button, dropdown) {
  const wasOpen = dropdown.classList.contains('show');
  closeAllJobMenus();
  if (!wasOpen) {
    positionJobMenuDropdown(button, dropdown);
  }
}

export function ensureJobMenuListeners() {
  if (window._jobMenuListenerAttached) return;
  document.addEventListener('click', (e) => {
    if (!e.target.closest('.job-menu')) {
      closeAllJobMenus();
    }
  });
  const tabContent = document.querySelector('.tab-content');
  if (tabContent) {
    tabContent.addEventListener('scroll', closeAllJobMenus, { passive: true });
  }
  window.addEventListener('resize', closeAllJobMenus);
  window._jobMenuListenerAttached = true;
}

export async function updateStatus() {
  try {
    const status = await invoke('get_status');
    updateStatusFromEvent(status);
  } catch (e) {
    console.error('Failed to update status:', e);
  }
}

export function resolveCurrentJobName(currentJobId) {
  if (!currentJobId) return null;
  const job = state.allJobs.find(j => j.id === currentJobId);
  return job ? job.name : null;
}

export function formatCurrentJobLabel(currentJobId) {
  return resolveCurrentJobName(currentJobId) || 'None';
}

export function updateStatusFromEvent(status) {
  const indicator = document.getElementById('runningIndicator');
  const hasActiveJob = !!status.current_job;

  if (indicator) indicator.classList.toggle('stopped', !hasActiveJob);

  const schedulerStatus = document.getElementById('schedulerStatus');
  if (schedulerStatus) schedulerStatus.textContent = hasActiveJob ? 'Running' : 'Idle';
  const queueSizeEl = document.getElementById('queueSize');
  if (queueSizeEl) queueSizeEl.textContent = status.queue_size || 0;
  const currentJobEl = document.getElementById('currentJob');
  if (currentJobEl) currentJobEl.textContent = formatCurrentJobLabel(status.current_job);
  const lastTickEl = document.getElementById('lastTick');
  if (lastTickEl) lastTickEl.textContent = status.last_tick ? new Date(status.last_tick).toLocaleString() : 'Never';

  state.currentRunningJob = resolveCurrentJobName(status.current_job);

  const queue = status.queue_size || 0;

  const sidebarStatus = document.getElementById('sidebarSchedulerStatus');
  const sidebarLabel = document.getElementById('sidebarSchedulerLabel');
  const sidebarCurrentJob = document.getElementById('sidebarCurrentJob');
  const sidebarQueueSize = document.getElementById('sidebarQueueSize');

  if (sidebarStatus) sidebarStatus.classList.toggle('idle', !hasActiveJob);
  if (sidebarLabel) sidebarLabel.textContent = hasActiveJob ? 'Running' : 'Idle';
  if (sidebarCurrentJob) sidebarCurrentJob.textContent = formatCurrentJobLabel(status.current_job);
  if (sidebarQueueSize) sidebarQueueSize.textContent = queue;

  const statQueueValue = document.getElementById('statQueueValue');
  const statQueueMeta = document.getElementById('statQueueMeta');
  if (statQueueValue) statQueueValue.textContent = queue;
  if (statQueueMeta) statQueueMeta.textContent = queue === 1 ? '1 waiting' : `${queue} waiting`;

  updateJobCounts(state.allJobs);
  refreshJobRowStates();
}

export function refreshJobRowStates() {
  document.querySelectorAll('#jobsTableBody tr[data-job-id]').forEach(row => {
    const job = state.allJobs.find(j => j.id === row.dataset.jobId);
    if (!job) return;
    const state = getJobRowState(job);
    const dot = row.querySelector('.job-status-dot');
    if (dot) dot.className = `job-status-dot ${state.dot}`;
    const primary = row.querySelector('.job-name-primary');
    const block = row.querySelector('.job-name-block');
    if (primary) {
      primary.textContent = job.name;
      primary.classList.toggle('running', state.running);
    }
    let sub = row.querySelector('.job-name-sub');
    if (state.sub) {
      if (!sub && block) {
        sub = document.createElement('span');
        sub.className = `job-name-sub ${state.running ? 'progress' : ''}`;
        block.appendChild(sub);
      }
      if (sub) {
        sub.textContent = state.sub;
        sub.className = `job-name-sub ${state.running ? 'progress' : ''}`;
        sub.style.display = '';
      }
    } else if (sub) {
      sub.remove();
    }
  });
}
