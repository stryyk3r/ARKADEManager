import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';
import { state } from '../state.js';
import { formatFileSize } from '../utils/format.js';

function getDataLookupType() {
  const selected = document.querySelector('input[name="dataLookupType"]:checked');
  return selected ? selected.value : 'profile';
}

function updateDataLookupFormLabels() {
  const lookupType = getDataLookupType();
  const isProfile = lookupType === 'profile';
  const label = document.getElementById('dataLookupIdentifierLabel');
  const input = document.getElementById('dataLookupIdentifier');
  const hint = document.getElementById('dataLookupHint');

  if (label) {
    label.textContent = isProfile ? 'EOSID' : 'TribeID';
  }
  if (input) {
    input.placeholder = isProfile
      ? '32-character hexadecimal EOSID'
      : 'Numeric tribe ID';
  }
  if (hint) {
    hint.textContent = isProfile
      ? 'Searches ASA servers under your configured server root for .arkprofile files.'
      : 'Searches ASA servers under your configured server root for .arktribe files.';
  }
}

export function setupDataLookupForm() {
  document.querySelectorAll('input[name="dataLookupType"]').forEach(radio => {
    radio.addEventListener('change', () => {
      updateDataLookupFormLabels();
      clearDataLookupResults();
    });
  });

  const input = document.getElementById('dataLookupIdentifier');
  if (input) {
    input.addEventListener('keydown', (event) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        searchDataLookup();
      }
    });
  }

  updateDataLookupFormLabels();
}

function validateDataLookupIdentifier(lookupType, identifier) {
  const trimmed = identifier.trim();
  if (!trimmed) {
    return lookupType === 'profile'
      ? 'EOSID is required'
      : 'TribeID is required';
  }

  if (lookupType === 'profile') {
    if (trimmed.length !== 32) {
      return 'EOSID must be exactly 32 characters';
    }
    if (!/^[0-9a-fA-F]+$/.test(trimmed)) {
      return 'EOSID must contain only hexadecimal characters';
    }
    return null;
  }

  if (!/^\d+$/.test(trimmed)) {
    return 'TribeID must be a positive integer';
  }
  if (trimmed === '0') {
    return 'TribeID must be greater than 0';
  }
  return null;
}

function clearDataLookupResults() {
  state.dataLookupResults = [];
  const emptyState = document.getElementById('dataLookupEmptyState');
  const table = document.getElementById('dataLookupResultsTable');
  const tbody = document.getElementById('dataLookupResultsBody');
  const countBadge = document.getElementById('dataLookupResultsCount');
  const selectAll = document.getElementById('dataLookupSelectAll');

  if (emptyState) {
    emptyState.hidden = false;
    emptyState.textContent = 'Enter an ID and click Search to find files.';
  }
  if (table) table.hidden = true;
  if (tbody) tbody.innerHTML = '';
  if (countBadge) countBadge.textContent = '0 matches';
  if (selectAll) {
    selectAll.checked = false;
    selectAll.indeterminate = false;
  }
  updateDataLookupDeleteBtn();
}

function formatDataLookupDate(isoDate) {
  if (!isoDate) return '—';
  const date = new Date(isoDate);
  if (Number.isNaN(date.getTime())) return '—';
  return date.toLocaleString();
}

function renderDataLookupResults(results) {
  state.dataLookupResults = results;
  const emptyState = document.getElementById('dataLookupEmptyState');
  const table = document.getElementById('dataLookupResultsTable');
  const tbody = document.getElementById('dataLookupResultsBody');
  const countBadge = document.getElementById('dataLookupResultsCount');
  const selectAll = document.getElementById('dataLookupSelectAll');

  if (!emptyState || !table || !tbody || !countBadge) {
    return;
  }

  countBadge.textContent = `${results.length} match${results.length === 1 ? '' : 'es'}`;

  if (results.length === 0) {
    emptyState.hidden = false;
    emptyState.textContent = 'No matching files found on configured ARK servers.';
    table.hidden = true;
    tbody.innerHTML = '';
    if (selectAll) {
      selectAll.checked = false;
      selectAll.indeterminate = false;
    }
    updateDataLookupDeleteBtn();
    return;
  }

  emptyState.hidden = true;
  table.hidden = false;
  tbody.innerHTML = '';

  results.forEach((result, index) => {
    const row = document.createElement('tr');

    const checkboxCell = document.createElement('td');
    checkboxCell.className = 'data-lookup-checkbox-col';
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.className = 'data-lookup-row-checkbox';
    checkbox.dataset.index = String(index);
    checkbox.addEventListener('change', updateDataLookupDeleteBtn);
    checkboxCell.appendChild(checkbox);

    const serverCell = document.createElement('td');
    serverCell.textContent = result.job_name;

    const mapCell = document.createElement('td');
    mapCell.textContent = result.map;

    const fileCell = document.createElement('td');
    fileCell.className = 'data-lookup-file-cell';
    fileCell.title = result.file_path;
    fileCell.textContent = result.file_name;

    const sizeCell = document.createElement('td');
    sizeCell.textContent = formatFileSize(result.file_size);

    const modifiedCell = document.createElement('td');
    modifiedCell.textContent = formatDataLookupDate(result.modified_at);

    row.appendChild(checkboxCell);
    row.appendChild(serverCell);
    row.appendChild(mapCell);
    row.appendChild(fileCell);
    row.appendChild(sizeCell);
    row.appendChild(modifiedCell);
    tbody.appendChild(row);
  });

  if (selectAll) {
    selectAll.checked = false;
    selectAll.indeterminate = false;
  }
  updateDataLookupDeleteBtn();
}

export async function searchDataLookup() {
  const lookupType = getDataLookupType();
  const input = document.getElementById('dataLookupIdentifier');
  const searchBtn = document.getElementById('dataLookupSearchBtn');
  const identifier = input ? input.value : '';
  const validationError = validateDataLookupIdentifier(lookupType, identifier);

  if (validationError) {
    alert(validationError);
    return;
  }

  if (searchBtn) {
    searchBtn.disabled = true;
    searchBtn.textContent = 'Searching...';
  }

  try {
    const results = await invoke('lookup_data_files', {
      lookupType,
      identifier: identifier.trim()
    });
    renderDataLookupResults(results);
  } catch (e) {
    console.error('Data lookup failed:', e);
    alert('Search failed: ' + e);
  } finally {
    if (searchBtn) {
      searchBtn.disabled = false;
      searchBtn.textContent = 'Search';
    }
  }
}

export function toggleDataLookupSelectAll() {
  const selectAll = document.getElementById('dataLookupSelectAll');
  const checkboxes = document.querySelectorAll('.data-lookup-row-checkbox');
  if (!selectAll) return;

  checkboxes.forEach(checkbox => {
    checkbox.checked = selectAll.checked;
  });
  selectAll.indeterminate = false;
  updateDataLookupDeleteBtn();
}

function updateDataLookupDeleteBtn() {
  const deleteBtn = document.getElementById('dataLookupDeleteBtn');
  const selectAll = document.getElementById('dataLookupSelectAll');
  const checkboxes = Array.from(document.querySelectorAll('.data-lookup-row-checkbox'));
  const checkedCount = checkboxes.filter(checkbox => checkbox.checked).length;

  if (deleteBtn) {
    deleteBtn.disabled = checkedCount === 0;
    deleteBtn.textContent = checkedCount > 0
      ? `Delete Selected (${checkedCount})`
      : 'Delete Selected';
  }

  if (selectAll && checkboxes.length > 0) {
    selectAll.checked = checkedCount === checkboxes.length;
    selectAll.indeterminate = checkedCount > 0 && checkedCount < checkboxes.length;
  }
}

export async function deleteSelectedDataLookupFiles() {
  const checkboxes = Array.from(document.querySelectorAll('.data-lookup-row-checkbox:checked'));
  if (checkboxes.length === 0) {
    alert('Select at least one file to delete.');
    return;
  }

  const selectedResults = checkboxes
    .map(checkbox => state.dataLookupResults[Number(checkbox.dataset.index)])
    .filter(Boolean);

  const summaryLines = selectedResults
    .slice(0, 8)
    .map(result => `${result.job_name} — ${result.file_name}`)
    .join('\n');
  const overflow = selectedResults.length > 8
    ? `\n...and ${selectedResults.length - 8} more`
    : '';

  const confirmed = await confirm(
    `Permanently delete ${selectedResults.length} file(s)?\n\n` +
    `${summaryLines}${overflow}\n\n` +
    'This cannot be undone. Files may be locked if the server is running.',
    {
      title: 'Delete Data Files',
      kind: 'warning'
    }
  );

  if (!confirmed) {
    return;
  }

  const deleteBtn = document.getElementById('dataLookupDeleteBtn');
  if (deleteBtn) {
    deleteBtn.disabled = true;
    deleteBtn.textContent = 'Deleting...';
  }

  try {
    const filePaths = selectedResults.map(result => result.file_path);
    const results = await invoke('delete_data_files', { filePaths });
    const failures = results.filter(result => !result.success);

    const remaining = state.dataLookupResults.filter(result => {
      const deleted = results.find(item => item.file_path === result.file_path);
      return !deleted || !deleted.success;
    });
    renderDataLookupResults(remaining);

    if (failures.length > 0) {
      const failureLines = failures
        .map(result => `${result.file_path}: ${result.error || 'Unknown error'}`)
        .join('\n');
      alert(`Deleted ${results.length - failures.length} file(s).\n\nFailed to delete:\n${failureLines}`);
    } else {
      alert(`Successfully deleted ${results.length} file(s).`);
    }
  } catch (e) {
    console.error('Delete failed:', e);
    alert('Delete failed: ' + e);
  } finally {
    updateDataLookupDeleteBtn();
  }
}

