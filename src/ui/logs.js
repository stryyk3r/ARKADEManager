import { invoke } from '@tauri-apps/api/core';
import { state } from '../state.js';
import { escapeHtml, onSegmentedChange, getSwitch, setSwitch, debounce } from '../utils/dom.js';
import { formatFileSize, formatDateTime } from '../utils/format.js';
import { toggleMenu, addMenuItem } from './menu.js';
import { toastError } from './feedback.js';

const LOG_LINES = 2000;
const MAX_RENDERED = 1500;
const SOURCE_ORDER = ['scheduler', 'backup', 'rcon', 'palworld', 'monthly', 'updater', 'plugins', 'lookup'];

// "2026-10-27T20:01:48.204123Z [INFO] arkade_manager::scheduler: message"
const LINE_RE = /^(\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-]\d{2}:?\d{2})?)\s+\[\s*([A-Za-z]+)\s*\]\s?(.*)$/;
// Older logs wrote warnings and errors without a time.
const NO_TIME_RE = /^\[\s*(TRACE|DEBUG|INFO|WARN|ERROR)\s*\]\s?(.*)$/;
const TARGET_RE = /^([A-Za-z_]\w*(?:::[A-Za-z_]\w*)+|arkade_manager):\s([\s\S]*)$/;

const $ = (id) => document.getElementById(id);

let suppressScrollEvent = false;

function sourceFor(target) {
  if (!target) return '';
  const t = target.toLowerCase();
  if (!t.startsWith('arkade_manager')) return t.split('::')[0];
  if (t.includes('rcon')) return 'rcon';
  if (t.includes('monthly')) return 'monthly';
  if (t.includes('palworld')) return 'palworld';
  if (t.includes('scheduler')) return 'scheduler';
  if (t.includes('updater')) return 'updater';
  if (t.includes('data_lookup')) return 'lookup';
  if (t.includes('plugins')) return 'plugins';
  if (t.includes('backup')) return 'backup';
  const parts = t.split('::');
  return parts[parts.length - 1];
}

function parseTime(text) {
  // JS Date only reliably parses milliseconds; trim longer fractions.
  const date = new Date(text.replace(/(\.\d{3})\d+/, '$1'));
  return Number.isNaN(date.getTime()) ? null : date;
}

function formatLogTime(date, fallback) {
  if (!date) return fallback || '';
  const p = (n, w = 2) => String(n).padStart(w, '0');
  return `${p(date.getHours())}:${p(date.getMinutes())}:${p(date.getSeconds())}.${p(date.getMilliseconds(), 3)}`;
}

function splitMessage(target, rest) {
  const m = TARGET_RE.exec(rest);
  return m ? { target: m[1], msg: m[2] } : { target: target || '', msg: rest };
}

export function parseLogs(raw) {
  const entries = [];
  for (const line of String(raw || '').split(/\r?\n/)) {
    if (!line.trim()) continue;
    let m = LINE_RE.exec(line);
    if (m) {
      const time = parseTime(m[1]);
      const { target, msg } = splitMessage('', m[3]);
      entries.push({ time, timeText: m[1], level: m[2].toUpperCase(), target, source: sourceFor(target), msg });
      continue;
    }
    m = NO_TIME_RE.exec(line);
    if (m) {
      const { target, msg } = splitMessage('', m[2]);
      const prev = entries[entries.length - 1];
      entries.push({ time: prev?.time ?? null, timeText: '', level: m[1].toUpperCase(), target, source: sourceFor(target), msg });
      continue;
    }
    // Continuation of a multi-line message, or a line in an unknown format.
    const prev = entries[entries.length - 1];
    if (prev && prev.level !== 'RAW') prev.msg += `\n${line}`;
    else entries.push({ time: null, timeText: '', level: 'RAW', target: '', source: '', msg: line });
  }
  for (const entry of entries) {
    entry.msg = entry.msg.replace(/[:;,]?\s+(caused by:)/gi, '\n$1');
  }
  return entries;
}

function levelBucket(level) {
  if (level === 'WARN') return 'warn';
  if (level === 'ERROR') return 'error';
  if (level === 'INFO') return 'info';
  return 'other';
}

function highlight(text, needle) {
  const safe = escapeHtml(text);
  if (!needle) return safe;
  const escapedNeedle = escapeHtml(needle).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return safe.replace(new RegExp(escapedNeedle, 'gi'), (m) => `<mark>${m}</mark>`);
}

function visibleEntries() {
  const { source, text } = state.logFilter;
  const needle = text.trim().toLowerCase();
  const clearedAt = state.logClearedAt;
  return state.logEntries.filter(e => {
    if (clearedAt !== null && (!e.time || e.time.getTime() <= clearedAt)) return false;
    if (source !== 'all' && e.source !== source) return false;
    if (needle && !`${e.msg} ${e.source} ${e.target}`.toLowerCase().includes(needle)) return false;
    return true;
  });
}

function updateSourceOptions() {
  const select = $('logSourceFilter');
  const found = new Set(state.logEntries.map(e => e.source).filter(Boolean));
  const sources = [...SOURCE_ORDER, ...[...found].filter(s => !SOURCE_ORDER.includes(s)).sort()];
  const wanted = ['all', ...sources].join('|');
  if (select.dataset.options === wanted) return;
  select.dataset.options = wanted;
  const current = state.logFilter.source;
  select.innerHTML = '<option value="all">All sources</option>'
    + sources.map(s => `<option value="${escapeHtml(s)}">${escapeHtml(s)}</option>`).join('');
  select.value = sources.includes(current) ? current : 'all';
  select._syncSelect?.();
}

function renderLogs() {
  const body = $('logsContent');
  const filtered = visibleEntries();

  const counts = { all: filtered.length, info: 0, warn: 0, error: 0 };
  filtered.forEach(e => {
    const bucket = levelBucket(e.level);
    if (bucket in counts) counts[bucket] += 1;
  });
  document.querySelectorAll('#logLevelFilter [data-count]').forEach(el => {
    el.textContent = counts[el.dataset.count].toLocaleString();
  });

  const level = state.logFilter.level;
  let shown = level === 'all' ? filtered : filtered.filter(e => levelBucket(e.level) === level);
  const truncated = shown.length > MAX_RENDERED;
  if (truncated) shown = shown.slice(-MAX_RENDERED);
  const needle = state.logFilter.text.trim();

  let html;
  if (state.logEntries.length === 0) {
    html = '<div class="empty compact">This log file is empty.</div>';
  } else if (shown.length === 0) {
    html = `<div class="empty compact">${state.logClearedAt !== null ? 'View cleared. New lines appear here as they are written.' : 'No lines match these filters.'}</div>`;
  } else {
    html = (truncated ? `<div class="empty compact" style="padding:8px">Showing the last ${MAX_RENDERED.toLocaleString()} matching lines.</div>` : '')
      + shown.map(e => {
        const lvl = e.level === 'RAW' ? '' : e.level;
        return `<div class="log-line log-grid lvl-${lvl.toLowerCase() || 'raw'}">`
          + `<span class="log-time" title="${escapeHtml(e.time ? e.time.toLocaleString() : '')}">${escapeHtml(formatLogTime(e.time, e.timeText))}</span>`
          + `<span class="log-level">${escapeHtml(lvl)}</span>`
          + `<span class="log-source" title="${escapeHtml(e.target)}">${escapeHtml(e.source)}</span>`
          + `<span class="log-msg">${highlight(e.msg, needle)}</span></div>`;
      }).join('');
  }

  if (body._html === html) return;
  const atBottom = body.scrollHeight - body.scrollTop - body.clientHeight < 40;
  body.innerHTML = html;
  body._html = html;
  if (state.logFilter.follow || atBottom) scrollToEnd();
}

function scrollToEnd() {
  const body = $('logsContent');
  suppressScrollEvent = true;
  body.scrollTop = body.scrollHeight;
  requestAnimationFrame(() => { suppressScrollEvent = false; });
}

function updateFollowState() {
  const follow = state.logFilter.follow;
  setSwitch($('logFollowSwitch'), follow);
  const isCurrent = !state.logFile || state.logFiles.find(f => f.name === state.logFile)?.current;
  $('logFollowState').classList.toggle('paused', !follow || !isCurrent);
  $('logFollowText').textContent = !isCurrent
    ? 'Viewing an earlier log'
    : follow ? 'Following · refreshes every 5s' : 'Paused · refreshes every 5s';
}

function updateFileTrigger() {
  const selected = state.logFiles.find(f => f.name === state.logFile) || state.logFiles.find(f => f.current) || state.logFiles[0];
  $('logFileName').textContent = selected ? selected.name : 'No log files';
  $('logFileCurrentTag').hidden = !selected?.current;
}

export async function loadLogFiles() {
  try {
    state.logFiles = await invoke('list_log_files');
  } catch (e) {
    state.logFiles = [];
    console.error('Failed to list log files:', e);
  }
  if (state.logFile && !state.logFiles.some(f => f.name === state.logFile)) state.logFile = null;
  updateFileTrigger();
  updateFollowState();
}

function isLogsVisible() {
  return $('logs')?.classList.contains('active');
}

export async function refreshLogs({ force = false } = {}) {
  if (!force && !isLogsVisible()) return;
  try {
    const raw = await invoke('read_logs', { lines: LOG_LINES, file: state.logFile });
    if (raw === state.logRaw && !force) return;
    state.logRaw = raw;
    state.logEntries = parseLogs(raw);
    updateSourceOptions();
    renderLogs();
  } catch (e) {
    console.error('Failed to refresh logs:', e);
  }
}

export function clearLogsView() {
  const last = state.logEntries[state.logEntries.length - 1];
  state.logClearedAt = last?.time ? last.time.getTime() : Date.now();
  renderLogs();
}

export async function openLogsFolder() {
  try {
    await invoke('open_logs_folder');
  } catch (e) {
    toastError('Could not open the logs folder', e);
  }
}

function openLogFileMenu() {
  const trigger = $('logFileTrigger');
  toggleMenu(trigger, (menu) => {
    menu.classList.add('log-file-menu', 'select-menu');
    menu.style.minWidth = `${Math.max(trigger.offsetWidth, 320)}px`;
    if (state.logFiles.length === 0) {
      addMenuItem(menu, 'No log files yet', null, { disabled: true });
      return;
    }
    const active = state.logFile || state.logFiles.find(f => f.current)?.name;
    state.logFiles.forEach(file => {
      const html = `<span>${escapeHtml(file.name)}</span>`
        + (file.current ? ' <span class="current-tag">current</span>' : '')
        + `<span class="size">${escapeHtml(formatFileSize(file.size))}</span>`;
      const item = addMenuItem(menu, file.name, () => selectLogFile(file), { selected: file.name === active, html });
      item.title = file.modified ? `Last written ${formatDateTime(file.modified)}` : '';
    });
  }, { align: 'right' });
}

async function selectLogFile(file) {
  state.logFile = file.current ? null : file.name;
  state.logClearedAt = null;
  state.logRaw = null;
  updateFileTrigger();
  updateFollowState();
  await refreshLogs({ force: true });
  scrollToEnd();
}

export async function showLogsTab() {
  await loadLogFiles();
  await refreshLogs({ force: true });
  if (state.logFilter.follow) scrollToEnd();
}

export function setupLogs(logsDir) {
  if (logsDir) {
    $('logsDirPath').textContent = logsDir;
    $('logsDirPath').title = logsDir;
  }
  onSegmentedChange($('logLevelFilter'), (value) => {
    state.logFilter.level = value;
    renderLogs();
  });
  $('logSourceFilter').addEventListener('change', (e) => {
    state.logFilter.source = e.target.value;
    renderLogs();
  });
  const applyText = debounce(() => renderLogs(), 150);
  $('logTextFilter').addEventListener('input', (e) => {
    state.logFilter.text = e.target.value;
    applyText();
  });
  $('logFollowSwitch').addEventListener('click', () => {
    state.logFilter.follow = !getSwitch($('logFollowSwitch'));
    updateFollowState();
    if (state.logFilter.follow) scrollToEnd();
  });
  $('logsContent').addEventListener('scroll', () => {
    if (suppressScrollEvent) return;
    const body = $('logsContent');
    const atBottom = body.scrollHeight - body.scrollTop - body.clientHeight < 8;
    if (state.logFilter.follow !== atBottom) {
      state.logFilter.follow = atBottom;
      updateFollowState();
    }
  }, { passive: true });
  $('logFileTrigger').addEventListener('click', (e) => {
    e.stopPropagation();
    openLogFileMenu();
  });
  updateFollowState();
}
