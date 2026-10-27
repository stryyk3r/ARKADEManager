import { invoke } from '@tauri-apps/api/core';
import { state, GAMES, JOB_GROUP_ORDER, getJobGroupKey, gameChipHtml } from '../../state.js';
import { escapeHtml, storageGet, storageSet } from '../../utils/dom.js';
import {
  formatFileSize, formatInterval, formatClockOrDate, formatRelative, formatUntil, formatFullDate, formatTime, parseDate, plural,
} from '../../utils/format.js';
import { toggleMenu, addMenuItem, addMenuDivider } from '../menu.js';
import { applyJobFilters } from './filters.js';
import { runJobNow, openBackupLocation, openEditDrawer, setJobEnabled, deleteJob } from './form.js';

const COLLAPSED_KEY = 'arkade.collapsedGroups';

const CHEVRON = '<svg class="group-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>';
const DOTS = '<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="5" cy="12" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="19" cy="12" r="1.8"/></svg>';

export function loadCollapsedGroups() {
  state.collapsedGroups = new Set(storageGet(COLLAPSED_KEY, []));
}

function saveCollapsedGroups() {
  storageSet(COLLAPSED_KEY, [...state.collapsedGroups]);
}

// ---------- Progress ----------

export function showBackupProgress(jobName, percent) {
  state.backupProgress = { jobName, percent };
  refreshJobRowStates();
  updateStatusStrip();
}

export function hideBackupProgress() {
  state.backupProgress = null;
  refreshJobRowStates();
  updateStatusStrip();
}

// ---------- Row state ----------

function isWarning(error) {
  const lower = String(error).toLowerCase();
  return lower.includes('completed with warnings') || lower.includes('warning');
}

function firstLine(text) {
  return String(text || '').split(/\r?\n/)[0].trim();
}

export function isJobRunning(job) {
  return !!state.status?.current_job && state.status.current_job === job.id;
}

/** kind: 'running' | 'paused' | 'error' | 'warning' | 'scheduled' */
export function getJobRowState(job) {
  if (isJobRunning(job)) return { kind: 'running', sub: 'Saving' };
  if (!job.enabled) return { kind: 'paused', sub: 'Paused' };
  if (job.last_error) {
    return isWarning(job.last_error)
      ? { kind: 'warning', sub: 'Completed with warnings' }
      : { kind: 'error', sub: firstLine(job.last_error) || 'Backup failed' };
  }
  return { kind: 'scheduled', sub: '' };
}

function progressFor(job) {
  const p = state.backupProgress;
  return p && p.jobName === job.name ? p.percent : null;
}

// ---------- Rendering ----------

function timeCell(primaryHtml, rel, title) {
  return `<div class="cell-time" title="${escapeHtml(title || '')}">${primaryHtml}${rel ? ` <span class="rel">${escapeHtml(rel)}</span>` : ''}</div>`;
}

function nextCellHtml(job, rs, now) {
  if (rs.kind === 'running') {
    const pct = progressFor(job);
    if (pct === null) return '<div class="cell-time"><span class="progress-pct">Saving…</span></div>';
    return `<div class="progress"><div class="progress-track"><div class="progress-fill" style="width:${pct}%"></div></div><span class="progress-pct">${pct}%</span></div>`;
  }
  if (rs.kind === 'paused' || !job.next_run_at) return '<div class="cell-time"><span class="rel">—</span></div>';
  const rel = formatUntil(job.next_run_at, now);
  return timeCell(
    `<span class="t">${escapeHtml(formatClockOrDate(job.next_run_at, new Date(now)))}</span>`,
    rs.kind === 'error' && rel !== 'due' ? `retry ${rel}` : rel,
    formatFullDate(job.next_run_at),
  );
}

function lastCellHtml(job, rs, now) {
  if (!job.last_run_at) return '<div class="cell-time"><span class="rel">Never</span></div>';
  if (rs.kind === 'error') {
    return timeCell('<span class="failed">Failed</span>', formatClockOrDate(job.last_run_at, new Date(now)), formatFullDate(job.last_run_at));
  }
  return timeCell(
    `<span class="t">${escapeHtml(formatClockOrDate(job.last_run_at, new Date(now)))}</span>`,
    formatRelative(job.last_run_at, now),
    formatFullDate(job.last_run_at),
  );
}

function jobRowInnerHtml(job, rs, now) {
  const subClass = { running: 'running', error: 'error', warning: 'warning' }[rs.kind] || '';
  const dotClass = rs.kind === 'scheduled' ? '' : rs.kind;
  const menuOpen = state.jobMenuJobId === job.id;
  return `
    <div class="job-name-cell">
      <span class="status-dot ${dotClass}"></span>
      <span class="job-name" title="${escapeHtml(job.name)}">${escapeHtml(job.name)}</span>
      ${rs.sub ? `<span class="job-sub ${subClass}" title="${escapeHtml(rs.kind === 'error' || rs.kind === 'warning' ? job.last_error : rs.sub)}">${escapeHtml(rs.sub)}</span>` : ''}
    </div>
    <div class="cell-muted col-interval">${escapeHtml(formatInterval(job.interval_value, job.interval_unit))}</div>
    ${nextCellHtml(job, rs, now)}
    ${lastCellHtml(job, rs, now)}
    <div class="cell-size">${job.last_file_size ? escapeHtml(formatFileSize(job.last_file_size)) : '<span class="rel" style="color:var(--text-muted);font-weight:500">—</span>'}</div>
    <div class="cell-menu">
      <button class="row-menu-btn" type="button" data-job-menu="${escapeHtml(job.id)}" aria-haspopup="menu" aria-expanded="${menuOpen}" aria-label="Actions for ${escapeHtml(job.name)}">${DOTS}</button>
    </div>`;
}

function rowClassName(rs, jobId) {
  return [
    'job-row jobs-grid',
    rs.kind === 'running' ? 'is-running' : '',
    rs.kind === 'paused' ? 'is-paused' : '',
    state.jobMenuJobId === jobId ? 'menu-open' : '',
  ].filter(Boolean).join(' ');
}

function updateRow(row, job, now) {
  const rs = getJobRowState(job);
  const html = jobRowInnerHtml(job, rs, now);
  if (row._html !== html) {
    const hadFocus = row.contains(document.activeElement);
    row.innerHTML = html;
    row._html = html;
    if (hadFocus) row.querySelector('.row-menu-btn')?.focus({ preventScroll: true });
  }
  const className = rowClassName(rs, job.id) + (row.classList.contains('hidden-row') ? ' hidden-row' : '');
  if (row.className !== className) row.className = className;
}

function groupMeta(groupKey, jobs) {
  const parts = [plural(jobs.length, 'job')];
  const failed = jobs.filter(j => getJobRowState(j).kind === 'error').length;
  const warnings = jobs.filter(j => getJobRowState(j).kind === 'warning').length;
  if (failed) parts.push(`${failed} failed`);
  if (warnings) parts.push(plural(warnings, 'warning'));
  if (!failed && !warnings && GAMES[groupKey]?.jobType === 'ark') {
    const clusters = [...new Set(jobs.map(j => j.monthly_cluster).filter(Boolean))];
    if (clusters.length) parts.push(clusters.join(', '));
  }
  return parts.join(' · ');
}

function groupHeaderInnerHtml(groupKey, jobs) {
  const size = jobs.reduce((sum, j) => sum + (j.last_file_size || 0), 0);
  return `
    <span class="group-title">
      ${CHEVRON}
      ${gameChipHtml(groupKey)}
      <span class="group-name">${escapeHtml(GAMES[groupKey].label)}</span>
      <span class="group-meta">${escapeHtml(groupMeta(groupKey, jobs))}</span>
    </span>
    <span class="group-size">${size ? escapeHtml(formatFileSize(size)) : ''}</span>
    <span></span>`;
}

function groupedJobs() {
  const grouped = {};
  for (const job of state.allJobs) {
    const key = getJobGroupKey(job);
    (grouped[key] ||= []).push(job);
  }
  return grouped;
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
  const hasAny = state.allJobs.length > 0;
  document.getElementById('backupEmptyAll').hidden = hasAny;
  document.getElementById('backupToolbar').hidden = !hasAny;
  document.getElementById('jobsPanel').hidden = !hasAny;
  document.getElementById('configuredJobsBadge').textContent = plural(state.allJobs.length, 'job');

  const body = document.getElementById('jobsTableBody');
  const scrollTop = body.scrollTop;
  body.innerHTML = '';
  const grouped = groupedJobs();
  const now = Date.now();

  for (const groupKey of JOB_GROUP_ORDER) {
    const groupJobs = grouped[groupKey];
    if (!groupJobs?.length) continue;
    groupJobs.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' }));

    const group = document.createElement('div');
    group.className = 'job-group';
    group.dataset.groupKey = groupKey;
    const collapsed = state.collapsedGroups.has(groupKey);
    group.classList.toggle('collapsed', collapsed);

    const header = document.createElement('button');
    header.type = 'button';
    header.className = 'group-row jobs-grid';
    header.dataset.toggleGroup = groupKey;
    header.setAttribute('aria-expanded', String(!collapsed));
    header.innerHTML = groupHeaderInnerHtml(groupKey, groupJobs);
    group.appendChild(header);

    const list = document.createElement('div');
    list.className = 'group-jobs';
    for (const job of groupJobs) {
      const row = document.createElement('div');
      row.dataset.jobId = job.id;
      row.dataset.group = groupKey;
      updateRow(row, job, now);
      list.appendChild(row);
    }
    group.appendChild(list);
    body.appendChild(group);
  }

  const noMatch = document.createElement('div');
  noMatch.className = 'no-match';
  noMatch.id = 'jobsNoMatch';
  noMatch.textContent = 'No jobs match these filters.';
  noMatch.hidden = true;
  body.appendChild(noMatch);
  body.scrollTop = scrollTop;

  applyJobFilters();
  updateStatusStrip();
}

/** Refresh row contents in place (status tick, progress, relative times). */
export function refreshJobRowStates() {
  const now = Date.now();
  document.querySelectorAll('#jobsTableBody .job-row[data-job-id]').forEach(row => {
    const job = state.allJobs.find(j => j.id === row.dataset.jobId);
    if (job) updateRow(row, job, now);
  });
  const grouped = groupedJobs();
  document.querySelectorAll('#jobsTableBody .job-group').forEach(group => {
    const jobs = grouped[group.dataset.groupKey] || [];
    const meta = group.querySelector('.group-meta');
    const text = groupMeta(group.dataset.groupKey, jobs);
    if (meta && meta.textContent !== text) meta.textContent = text;
  });
  // Row states change over time, so a status filter has to be re-applied.
  if (state.jobFilter.status !== 'all') applyJobFilters();
}

// ---------- Collapse ----------

export function toggleGroup(groupKey) {
  if (state.collapsedGroups.has(groupKey)) state.collapsedGroups.delete(groupKey);
  else state.collapsedGroups.add(groupKey);
  saveCollapsedGroups();
  const group = document.querySelector(`#jobsTableBody .job-group[data-group-key="${groupKey}"]`);
  if (group) {
    const collapsed = state.collapsedGroups.has(groupKey);
    group.classList.toggle('collapsed', collapsed);
    group.querySelector('.group-row')?.setAttribute('aria-expanded', String(!collapsed));
  }
  updateToggleAllLabel();
}

function visibleGroupKeys() {
  return [...document.querySelectorAll('#jobsTableBody .job-group:not(.hidden-row)')].map(g => g.dataset.groupKey);
}

export function updateToggleAllLabel() {
  const btn = document.getElementById('toggleGroupsBtn');
  if (!btn) return;
  const keys = visibleGroupKeys();
  const allCollapsed = keys.length > 0 && keys.every(k => state.collapsedGroups.has(k));
  btn.textContent = allCollapsed ? 'Expand all' : 'Collapse all';
}

export function toggleAllGroups() {
  const keys = visibleGroupKeys();
  const allCollapsed = keys.length > 0 && keys.every(k => state.collapsedGroups.has(k));
  keys.forEach(k => (allCollapsed ? state.collapsedGroups.delete(k) : state.collapsedGroups.add(k)));
  saveCollapsedGroups();
  document.querySelectorAll('#jobsTableBody .job-group').forEach(group => {
    const collapsed = state.collapsedGroups.has(group.dataset.groupKey);
    group.classList.toggle('collapsed', collapsed);
    group.querySelector('.group-row')?.setAttribute('aria-expanded', String(!collapsed));
  });
  updateToggleAllLabel();
}

// ---------- Row menu ----------

function setMenuOpenFor(jobId) {
  state.jobMenuJobId = jobId;
  document.querySelectorAll('#jobsTableBody .job-row').forEach(row => {
    row.classList.toggle('menu-open', row.dataset.jobId === jobId);
  });
}

export function openJobMenu(button, jobId) {
  const job = state.allJobs.find(j => j.id === jobId);
  if (!job) return;
  const menu = toggleMenu(button, (m) => {
    addMenuItem(m, 'Run now', () => runJobNow(job.id), { disabled: isJobRunning(job) });
    addMenuItem(m, 'Open backup folder', () => openBackupLocation(job.destination_dir));
    addMenuItem(m, 'Edit job…', () => openEditDrawer(job.id));
    addMenuItem(m, job.enabled ? 'Pause schedule' : 'Resume schedule', () => setJobEnabled(job.id, !job.enabled));
    addMenuDivider(m);
    addMenuItem(m, 'Delete job', () => deleteJob(job.id), { danger: true });
  }, { onClose: () => setMenuOpenFor(null) });
  if (menu) setMenuOpenFor(jobId);
}

export function bindJobsTableEvents() {
  const body = document.getElementById('jobsTableBody');
  body.addEventListener('click', (e) => {
    const menuBtn = e.target.closest('[data-job-menu]');
    if (menuBtn) {
      e.stopPropagation();
      openJobMenu(menuBtn, menuBtn.dataset.jobMenu);
      return;
    }
    const groupBtn = e.target.closest('[data-toggle-group]');
    if (groupBtn) {
      toggleGroup(groupBtn.dataset.toggleGroup);
      return;
    }
  });
  body.addEventListener('dblclick', (e) => {
    const row = e.target.closest('.job-row[data-job-id]');
    if (row && !e.target.closest('button')) openEditDrawer(row.dataset.jobId);
  });
  document.getElementById('toggleGroupsBtn')?.addEventListener('click', toggleAllGroups);
}

// ---------- Status ----------

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
  return state.allJobs.find(j => j.id === currentJobId)?.name ?? null;
}

export function updateStatusFromEvent(status) {
  const previousJob = state.status?.current_job ?? null;
  state.status = status || {};
  const currentJob = state.status.current_job ?? null;
  state.currentRunningJob = resolveCurrentJobName(currentJob);
  if (previousJob && previousJob !== currentJob) state.backupProgress = null;

  const active = !!currentJob;
  const statusEl = document.getElementById('sidebarSchedulerStatus');
  statusEl?.classList.toggle('idle', !active);
  document.getElementById('sidebarSchedulerLabel').textContent = active ? 'Running' : 'Idle';
  const currentEl = document.getElementById('sidebarCurrentJob');
  currentEl.textContent = state.currentRunningJob || (active ? 'Starting…' : 'None');
  currentEl.title = currentEl.textContent;
  document.getElementById('sidebarQueueSize').textContent = String(state.status.queue_size || 0);
  const tick = parseDate(state.status.last_tick);
  document.getElementById('sidebarLastTick').textContent = tick ? tick.toLocaleTimeString() : '—';

  refreshJobRowStates();
  updateStatusStrip();
}

export function computeSuccessRate(jobs) {
  const enabled = (jobs || []).filter(j => j.enabled);
  if (enabled.length === 0) return '—';
  const dayAgo = Date.now() - 24 * 60 * 60 * 1000;
  const ok = enabled.filter(j => !j.last_error && j.last_run_at && new Date(j.last_run_at).getTime() > dayAgo).length;
  return `${Math.round((ok / enabled.length) * 100)}%`;
}

function setStatusValue(id, html, className = '') {
  const el = document.getElementById(id);
  if (!el) return;
  if (el._html !== html) {
    el.innerHTML = html;
    el._html = html;
  }
  el.className = `status-value ${className}`.trim();
}

export function updateStatusStrip() {
  const jobs = state.allJobs;
  const now = Date.now();

  const running = jobs.find(isJobRunning);
  if (running) {
    const pct = progressFor(running);
    setStatusValue('statRunning', `${escapeHtml(running.name)}${pct !== null ? ` <span class="sub">${pct}%</span>` : ''}`, 'is-running');
  } else if (state.status?.current_job) {
    setStatusValue('statRunning', 'Starting…', 'is-running');
  } else {
    setStatusValue('statRunning', 'Idle', 'is-muted');
  }

  setStatusValue('statQueue', String(state.status?.queue_size || 0));

  const upcoming = jobs
    .filter(j => j.enabled && j.next_run_at && !isJobRunning(j))
    .sort((a, b) => new Date(a.next_run_at) - new Date(b.next_run_at))[0];
  if (upcoming) {
    setStatusValue('statNext', `${escapeHtml(upcoming.name)} <span class="sub">${escapeHtml(formatUntil(upcoming.next_run_at, now))}</span>`);
    document.getElementById('statNext').title = `${upcoming.name} at ${formatTime(upcoming.next_run_at)}`;
  } else {
    setStatusValue('statNext', 'None scheduled', 'is-muted');
  }

  setStatusValue('statSuccess', computeSuccessRate(jobs));

  const kinds = jobs.map(j => getJobRowState(j).kind);
  const failed = kinds.filter(k => k === 'error').length;
  const warnings = kinds.filter(k => k === 'warning').length;
  if (!failed && !warnings) {
    setStatusValue('statAttention', 'Nothing', 'is-muted');
  } else {
    const parts = [];
    if (failed) parts.push(`<span class="is-failed">${failed} failed</span>`);
    if (warnings) parts.push(`<span class="is-warning">${parts.length ? '· ' : ''}${plural(warnings, 'warning')}</span>`);
    setStatusValue('statAttention', parts.join(' '));
  }
}
