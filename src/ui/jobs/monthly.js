import { invoke } from '@tauri-apps/api/core';
import { state, MONTHLY_CLUSTERS } from '../../state.js';
import { escapeHtml, setButtonBusy } from '../../utils/dom.js';
import { parseDate, plural } from '../../utils/format.js';
import { openOverlay, closeOverlay, isOverlayOpen, confirmDialog, toast, toastError } from '../feedback.js';

const $ = (id) => document.getElementById(id);

function formatCopyTimes(times) {
  const dates = times.map(parseDate).filter(Boolean);
  if (dates.length === 0) return '';
  const first = dates[0].toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
  if (dates.length === 1) return first;
  const sameDay = dates[0].toDateString() === dates[1].toDateString();
  const second = sameDay
    ? dates[1].toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
    : dates[1].toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
  return `${first} · ${second}`;
}

function rowFor(status) {
  const job = state.allJobs.find(j => j.id === status.job_id);
  const copies = Math.min(2, status.copied_this_month || 0);
  const times = status.copy_times || [];
  let text;
  let tone = '';
  if (copies >= 2) {
    text = formatCopyTimes(times);
  } else if (copies === 1) {
    tone = 'partial';
    const reason = !job?.enabled ? 'paused' : job?.last_error ? 'last run failed' : 'waiting for next backup';
    text = `1 of 2 · ${reason}`;
  } else {
    tone = 'missing';
    if (!status.monthly_cluster) text = 'No cluster set';
    else if (job?.last_error) text = 'No copies, last run failed';
    else if (!job?.enabled) text = 'No copies, job paused';
    else text = 'No copies yet';
  }
  const pip = (n) => (copies >= n ? 'full' : copies === 1 && n === 2 ? 'partial' : '');
  return `
    <div class="monthly-row">
      <span class="name" title="${escapeHtml(status.job_name)}">${escapeHtml(status.job_name)}</span>
      <span class="pips" aria-label="${copies} of 2 copies"><span class="pip ${pip(1)}"></span><span class="pip ${pip(2)}"></span></span>
      <span class="when ${tone}" title="${escapeHtml(text)}">${escapeHtml(text)}</span>
    </div>`;
}

function renderMonthlyStatus(result) {
  const jobs = result.jobs || [];
  const now = new Date();
  $('monthlyTitle').textContent = `Monthly archive · ${now.toLocaleDateString(undefined, { month: 'long', year: 'numeric' })}`;
  $('monthlySub').innerHTML = `First 2 backups per job copied to <span class="mono">${escapeHtml(result.destination_base || '')}</span>`;
  $('monthlySub').title = result.month_folder || '';

  const complete = jobs.filter(j => j.copied_this_month >= 2).length;
  const partial = jobs.filter(j => j.copied_this_month === 1).length;
  const missing = jobs.filter(j => !j.copied_this_month).length;
  $('monthlySummary').innerHTML = `
    <span><strong>${complete}</strong> <span>complete</span></span>
    <span><strong class="text-yellow">${partial}</strong> <span>partial</span></span>
    <span><strong class="text-red">${missing}</strong> <span>missing</span></span>`;

  if (jobs.length === 0) {
    $('monthlyList').innerHTML = '<div class="empty compact">No jobs yet.</div>';
    return;
  }

  const groups = new Map();
  for (const job of jobs) {
    const key = job.monthly_cluster || 'No cluster';
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(job);
  }
  const order = [...MONTHLY_CLUSTERS, ...[...groups.keys()].filter(k => !MONTHLY_CLUSTERS.includes(k))];
  $('monthlyList').innerHTML = order
    .filter(key => groups.has(key))
    .map(key => {
      const rows = groups.get(key)
        .sort((a, b) => a.job_name.localeCompare(b.job_name, undefined, { numeric: true }))
        .map(rowFor)
        .join('');
      return `<div class="monthly-group">${escapeHtml(key)}</div>${rows}`;
    })
    .join('');
}

async function loadMonthlyStatus() {
  try {
    renderMonthlyStatus(await invoke('get_monthly_status'));
  } catch (e) {
    $('monthlyList').innerHTML = `<div class="empty compact is-error">${escapeHtml(String(e))}</div>`;
  }
}

export async function showMonthlyStatus() {
  $('monthlyList').innerHTML = '<div class="empty compact">Loading…</div>';
  $('monthlySummary').innerHTML = '';
  openOverlay($('monthlyStatusModal'), { onDismiss: closeMonthlyStatus, initialFocus: '#monthlyRunBtn' });
  await loadMonthlyStatus();
}

export function closeMonthlyStatus() {
  closeOverlay($('monthlyStatusModal'));
}

async function executeMonthly(button) {
  setButtonBusy(button, true, 'Running…');
  try {
    const result = await invoke('run_monthly_archive');
    const archived = result?.archived ?? 0;
    toast(archived ? `Monthly archive: ${plural(archived, 'copy', 'copies')} made` : 'Monthly archive is already up to date', {
      kind: 'success',
      detail: result?.destination || '',
    });
  } catch (e) {
    toastError('Monthly archive failed', e);
  } finally {
    setButtonBusy(button, false);
  }
  if (isOverlayOpen($('monthlyStatusModal'))) await loadMonthlyStatus();
}

export async function runMonthlyBackup() {
  const confirmed = await confirmDialog({
    title: 'Run the monthly archive?',
    body: '<p>Copies the first 2 backups per job for this month into the monthly folder. Large copies can take a while.</p>',
    confirmLabel: 'Run monthly',
  });
  if (!confirmed) return;
  await executeMonthly(document.querySelector('[data-action="runMonthlyBackup"]'));
}

export async function runMonthlyFromStatus() {
  await executeMonthly($('monthlyRunBtn'));
}
