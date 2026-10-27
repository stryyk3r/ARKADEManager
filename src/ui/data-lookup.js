import { invoke } from '@tauri-apps/api/core';
import { state } from '../state.js';
import { escapeHtml, checkHtml, setCheck, onSegmentedChange, setButtonBusy } from '../utils/dom.js';
import { formatFileSize, formatDateTime, formatFullDate, plural } from '../utils/format.js';
import { confirmDialog, listHtml, toast } from './feedback.js';
import { asaServerRoot } from './plugins/servers.js';

const $ = (id) => document.getElementById(id);

const TYPES = {
  profile: { placeholder: '32-character EOS ID', ext: '.arkprofile' },
  tribe: { placeholder: 'Numeric tribe ID', ext: '.arktribe' },
};

/** { valid, hint, tone } for the current input. */
function validate(type, raw) {
  const value = raw.trim();
  if (type === 'profile') {
    if (!value) return { valid: false, hint: '', tone: '' };
    if (!/^[0-9a-fA-F]*$/.test(value)) return { valid: false, hint: 'hex only', tone: 'is-invalid' };
    if (value.length !== 32) return { valid: false, hint: `${value.length} / 32 hex`, tone: value.length > 32 ? 'is-invalid' : '' };
    return { valid: true, hint: '32 / 32 hex', tone: 'is-valid' };
  }
  if (!value) return { valid: false, hint: '', tone: '' };
  if (!/^\d+$/.test(value)) return { valid: false, hint: 'numbers only', tone: 'is-invalid' };
  if (/^0+$/.test(value)) return { valid: false, hint: 'must be above 0', tone: 'is-invalid' };
  return { valid: true, hint: 'valid ID', tone: 'is-valid' };
}

function updateValidation() {
  const result = validate(state.dataLookupType, $('dataLookupIdentifier').value);
  const hint = $('dataLookupHint');
  hint.textContent = result.hint;
  hint.className = `input-hint ${result.tone}`;
  $('dataLookupInputGroup').classList.toggle('is-invalid', result.tone === 'is-invalid');
  if ($('dataLookupSearchBtn').dataset.label === undefined) $('dataLookupSearchBtn').disabled = !result.valid;
  return result.valid;
}

function renderIdleMeta() {
  const ext = TYPES[state.dataLookupType].ext;
  $('dataLookupMeta').innerHTML = `Searches every ASA server under <span class="mono">${escapeHtml(asaServerRoot())}</span> for ${ext} files.`;
}

function clearResults() {
  state.dataLookupResults = [];
  state.lookupSelection.clear();
  $('dataLookupResultsHead').hidden = true;
  $('dataLookupTableHead').hidden = true;
  $('dataLookupBar').hidden = true;
  $('dataLookupPanel').style.marginTop = '22px';
  $('dataLookupResultsBody').innerHTML = '<div class="empty">Enter an ID and search to find files.</div>';
}

function renderResults() {
  const results = state.dataLookupResults;
  const body = $('dataLookupResultsBody');
  $('dataLookupResultsHead').hidden = false;
  $('dataLookupPanel').style.marginTop = '0';
  const servers = new Set(results.map(r => r.root_dir || r.job_name));
  const total = results.reduce((sum, r) => sum + (r.file_size || 0), 0);
  $('dataLookupResultsCount').textContent = results.length ? `${plural(results.length, 'file')} found` : 'No files found';
  $('dataLookupResultsMeta').textContent = results.length ? `on ${plural(servers.size, 'server')} · ${formatFileSize(total)}` : '';
  $('dataLookupTableHead').hidden = results.length === 0;
  $('dataLookupBar').hidden = results.length === 0;

  if (results.length === 0) {
    body.innerHTML = '<div class="empty">No files found for this ID.</div>';
    renderSelection();
    return;
  }
  body.innerHTML = results.map((r, i) => {
    const on = state.lookupSelection.has(r.file_path);
    return `<div class="lookup-row lookup-grid ${on ? 'selected' : ''}" data-index="${i}" role="row">
      <button class="check-btn" type="button" aria-pressed="${on}" aria-label="Select ${escapeHtml(r.file_name)} on ${escapeHtml(r.job_name)}">${checkHtml(on)}</button>
      <span class="server truncate" title="${escapeHtml(r.root_dir)}">${escapeHtml(r.job_name)}</span>
      <span class="map truncate col-map">${escapeHtml(r.map)}</span>
      <span class="file truncate" title="${escapeHtml(r.file_path)}">${escapeHtml(r.file_name)}</span>
      <span class="size">${escapeHtml(formatFileSize(r.file_size))}</span>
      <span class="modified col-modified" title="${escapeHtml(formatFullDate(r.modified_at))}">${escapeHtml(formatDateTime(r.modified_at))}</span>
    </div>`;
  }).join('');
  renderSelection();
}

function renderSelection() {
  const results = state.dataLookupResults;
  const selected = results.filter(r => state.lookupSelection.has(r.file_path));
  const n = selected.length;
  document.querySelectorAll('#dataLookupResultsBody .lookup-row').forEach(row => {
    const r = results[Number(row.dataset.index)];
    const on = state.lookupSelection.has(r.file_path);
    row.classList.toggle('selected', on);
    row.querySelector('.check-btn')?.setAttribute('aria-pressed', String(on));
    setCheck(row.querySelector('.check'), on);
  });
  setCheck($('dataLookupSelectAll').querySelector('.check'), n === 0 ? false : n === results.length ? true : 'mixed');
  $('dataLookupBar').classList.toggle('is-idle', n === 0);
  $('dataLookupSelectedCount').textContent = `${n} selected`;
  const size = selected.reduce((sum, r) => sum + (r.file_size || 0), 0);
  const servers = [...new Set(selected.map(r => r.job_name))];
  $('dataLookupSelectedDetail').textContent = n ? `${formatFileSize(size)} · ${servers.join(', ')}` : 'Select files to delete them.';
  $('dataLookupClearBtn').disabled = n === 0;
  const del = $('dataLookupDeleteBtn');
  if (del.dataset.label === undefined) {
    del.disabled = n === 0;
    del.textContent = n ? `Delete ${plural(n, 'file')}` : 'Delete files';
  }
}

export async function searchDataLookup() {
  if (!updateValidation()) return;
  const lookupType = state.dataLookupType;
  const identifier = $('dataLookupIdentifier').value.trim();
  const btn = $('dataLookupSearchBtn');
  setButtonBusy(btn, true, 'Searching…');
  const started = performance.now();
  try {
    const [results, roots] = await Promise.all([
      invoke('lookup_data_files', { lookupType, identifier }),
      invoke('get_plugin_server_roots').catch(() => null),
    ]);
    const seconds = ((performance.now() - started) / 1000).toFixed(1);
    state.dataLookupResults = results;
    state.lookupSelection.clear();
    $('dataLookupMeta').innerHTML = `Searched ${roots ? plural(roots.length, 'server') : 'servers'} under <span class="mono">${escapeHtml(asaServerRoot())}</span> in ${seconds}s`;
    renderResults();
  } catch (e) {
    $('dataLookupMeta').innerHTML = `<span class="text-red">${escapeHtml(String(e))}</span>`;
  } finally {
    setButtonBusy(btn, false);
    updateValidation();
  }
}

export function toggleDataLookupSelectAll() {
  const all = state.dataLookupResults.length > 0 && state.lookupSelection.size === state.dataLookupResults.length;
  state.lookupSelection.clear();
  if (!all) state.dataLookupResults.forEach(r => state.lookupSelection.add(r.file_path));
  renderSelection();
}

export async function deleteSelectedDataLookupFiles() {
  const selected = state.dataLookupResults.filter(r => state.lookupSelection.has(r.file_path));
  if (selected.length === 0) return;
  const confirmed = await confirmDialog({
    title: `Delete ${plural(selected.length, 'file')}?`,
    body: `${listHtml(selected.map(r => `${r.job_name} — ${r.file_name}`))}
      <p>This cannot be undone. Files may be locked while a server is running.</p>`,
    confirmLabel: `Delete ${plural(selected.length, 'file')}`,
    danger: true,
  });
  if (!confirmed) return;

  const btn = $('dataLookupDeleteBtn');
  setButtonBusy(btn, true, 'Deleting…');
  try {
    const results = await invoke('delete_data_files', { filePaths: selected.map(r => r.file_path) });
    const deleted = new Set(results.filter(r => r.success).map(r => r.file_path));
    const failures = results.filter(r => !r.success);
    state.dataLookupResults = state.dataLookupResults.filter(r => !deleted.has(r.file_path));
    deleted.forEach(p => state.lookupSelection.delete(p));
    if (failures.length) {
      toast(`Deleted ${deleted.size} of ${results.length} files`, {
        kind: 'error',
        detail: failures.map(f => `${f.file_path}: ${f.error || 'Unknown error'}`).join('\n'),
      });
    } else {
      toast(`Deleted ${plural(deleted.size, 'file')}`, { kind: 'success' });
    }
  } catch (e) {
    toast('Delete failed', { kind: 'error', detail: String(e) });
  } finally {
    setButtonBusy(btn, false);
    renderResults();
  }
}

export function setupDataLookupForm() {
  onSegmentedChange($('dataLookupType'), (value) => {
    state.dataLookupType = value;
    $('dataLookupIdentifier').placeholder = TYPES[value].placeholder;
    $('dataLookupIdentifier').value = '';
    updateValidation();
    clearResults();
    renderIdleMeta();
    $('dataLookupIdentifier').focus();
  });
  $('dataLookupIdentifier').addEventListener('input', updateValidation);
  $('dataLookupIdentifier').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      searchDataLookup();
    }
  });
  $('dataLookupSelectAll').addEventListener('click', toggleDataLookupSelectAll);
  $('dataLookupClearBtn').addEventListener('click', () => {
    state.lookupSelection.clear();
    renderSelection();
  });
  $('dataLookupResultsBody').addEventListener('click', (e) => {
    const row = e.target.closest('.lookup-row');
    if (!row) return;
    const r = state.dataLookupResults[Number(row.dataset.index)];
    if (!r) return;
    if (state.lookupSelection.has(r.file_path)) state.lookupSelection.delete(r.file_path);
    else state.lookupSelection.add(r.file_path);
    renderSelection();
  });
  updateValidation();
  renderIdleMeta();
}

/** Re-render text that depends on config (after Settings saves). */
export function refreshDataLookupMeta() {
  if (state.dataLookupResults.length === 0 && $('dataLookupResultsHead').hidden) renderIdleMeta();
}
