import { invoke } from '@tauri-apps/api/core';

export async function openLogsFolder() {
  try {
    await invoke('open_logs_folder');
  } catch (e) {
    alert('Failed to open logs folder: ' + e);
  }
}

export async function refreshLogs() {
  try {
    const logs = await invoke('read_logs', { lines: 100 });
    document.getElementById('logsContent').textContent = logs;
  } catch (e) {
    console.error('Failed to refresh logs:', e);
  }
}

export function clearLogsView() {
  document.getElementById('logsContent').textContent = '';
}
