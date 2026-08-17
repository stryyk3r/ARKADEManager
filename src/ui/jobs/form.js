import { invoke } from '@tauri-apps/api/core';
import { open, confirm } from '@tauri-apps/plugin-dialog';
import { state } from '../../state.js';
import { setSelectValue } from '../../utils/dom.js';
import { refreshMapSelects } from '../settings/maps.js';
import { refreshJobs } from './table.js';

export async function pickRootDir() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Server Root Directory'
    });
    if (selected) {
      document.getElementById('rootDir').value = selected;
      clearError('rootDirError');
    }
  } catch (e) {
    console.error('Failed to pick directory:', e);
    alert('Failed to open directory picker: ' + e.message);
  }
}

export async function pickDestinationDir() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Destination Directory'
    });
    if (selected) {
      document.getElementById('destinationDir').value = selected;
      clearError('destinationDirError');
    }
  } catch (e) {
    console.error('Failed to pick directory:', e);
    alert('Failed to open directory picker: ' + e.message);
  }
}

export function cancelJobForm() {
  const formContainer = document.getElementById('jobFormContainer');
  formContainer.style.display = 'none';
  formContainer.style.visibility = 'hidden';
  formContainer.style.height = '0';
  formContainer.style.overflow = 'hidden';
  formContainer.classList.remove('show');
  clearForm();
  state.currentJobId = null;
};

export async function addJob() {
  try {
    const job = collectJobData();
    if (!job) return;
    
    await invoke('add_job', { job });
    clearForm();
    const formContainer = document.getElementById('jobFormContainer');
    formContainer.style.display = 'none';
    formContainer.style.visibility = 'hidden';
    formContainer.style.height = '0';
    formContainer.style.overflow = 'hidden';
    formContainer.classList.remove('show');
    await refreshJobs();
  } catch (e) {
    alert('Failed to add job: ' + e);
  }
};

export async function updateJob(jobId) {
  if (!jobId) {
    jobId = state.currentJobId;
  }
  
  if (!jobId) {
    alert('Please select a job to update');
    return;
  }
  
  try {
    // Load job into form
    const jobs = await invoke('list_jobs');
    const job = jobs.find(j => j.id === jobId);
    if (!job) {
      alert('Job not found');
      return;
    }
    
    await loadJobIntoForm(job);
    const formContainer = document.getElementById('jobFormContainer');
    formContainer.style.display = 'block';
    formContainer.style.visibility = 'visible';
    formContainer.style.height = 'auto';
    formContainer.style.overflow = 'visible';
    formContainer.classList.add('show');
    document.getElementById('jobFormTitle').textContent = 'Edit Job';
    document.getElementById('submitJobBtn').textContent = 'Update Job';
    document.getElementById('submitJobBtn').onclick = async () => {
      try {
        const jobData = collectJobData();
        if (!jobData) return;
        
        jobData.id = state.currentJobId;
        await invoke('update_job', { job: jobData });
        clearForm();
        const formContainer = document.getElementById('jobFormContainer');
        formContainer.style.display = 'none';
        formContainer.style.visibility = 'hidden';
        formContainer.style.height = '0';
        formContainer.style.overflow = 'hidden';
        formContainer.classList.remove('show');
        await refreshJobs();
      } catch (e) {
        alert('Failed to update job: ' + e);
      }
    };
    document.getElementById('jobFormContainer').scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  } catch (e) {
    alert('Failed to load job: ' + e);
  }
};

export async function deleteJob(jobId) {
  if (!jobId) {
    jobId = state.currentJobId;
  }
  
  if (!jobId) {
    alert('Please select a job to delete');
    return;
  }
  
  // Use Tauri's confirm dialog which properly blocks execution
  const confirmed = await confirm('Are you sure you want to delete this job?', {
    title: 'Delete Job',
    kind: 'warning',
  });
  
  if (!confirmed) {
    return;
  }
  
  try {
    await invoke('delete_job', { id: jobId });
    if (state.currentJobId === jobId) {
      clearForm();
      const formContainer = document.getElementById('jobFormContainer');
      formContainer.style.display = 'none';
      formContainer.style.visibility = 'hidden';
      formContainer.style.height = '0';
      formContainer.style.overflow = 'hidden';
      formContainer.classList.remove('show');
    }
    await refreshJobs();
  } catch (e) {
    alert('Failed to delete job: ' + e);
  }
};

export async function runJobNow(jobId) {
  if (!jobId) {
    jobId = state.currentJobId;
  }
  
  if (!jobId) {
    alert('Please select a job to run');
    return;
  }
  
  try {
    await invoke('run_job_now', { id: jobId });
    alert('Job queued for execution');
  } catch (e) {
    alert('Failed to run job: ' + e);
  }
};

export async function openBackupLocation(destinationDir) {
  if (!destinationDir || !destinationDir.trim()) {
    alert('No backup location set for this job');
    return;
  }
  try {
    await invoke('open_backup_location', { path: destinationDir });
  } catch (e) {
    alert('Failed to open backup location: ' + e);
  }
};

export async function openLogsFolder() {
  try {
    await invoke('open_logs_folder');
  } catch (e) {
    alert('Failed to open logs folder: ' + e);
  }
};

export function clearForm() {
  document.getElementById('rootDir').value = '';
  document.getElementById('destinationDir').value = '';
  setSelectValue(document.getElementById('mapSelect'), '');
  document.getElementById('jobName').value = '';
  document.getElementById('jobNameMinecraft').value = '';
  document.getElementById('includeSaves').checked = false;
  document.getElementById('includeMap').checked = false;
  document.getElementById('includeServerFiles').checked = false;
  document.getElementById('includePluginConfigs').checked = false;
  document.getElementById('intervalValue').value = '1';
  document.getElementById('intervalUnit').value = 'minutes';
  document.getElementById('intervalValueMinecraft').value = '1';
  document.getElementById('intervalUnitMinecraft').value = 'minutes';
  document.getElementById('retentionDays').value = '7';
  document.getElementById('enabled').checked = false;
  setJobFormVisibilityForType('ark');
  clearAllErrors();
};

export async function showMonthlyStatus() {
  try {
    const status = await invoke('get_monthly_status');
    const jobs = status.jobs || [];
    const completed = jobs.filter(j => j.completed).length;
    const total = jobs.length;
    const lines = [];
    lines.push(`Monthly Status (current month)`);
    lines.push(`Month folder: ${status.month_folder}`);
    lines.push(`Completed: ${completed}/${total} job(s) have 2 monthly copies`);
    lines.push('');
    if (jobs.length === 0) {
      lines.push('No jobs found.');
    } else {
      for (const j of jobs) {
        const cluster = j.monthly_cluster || '(unset)';
        const count = (j.copied_this_month ?? 0);
        lines.push(`${j.completed ? 'COMPLETED' : 'PENDING'}  ${j.job_name}  [${cluster}]  ${count}/2`);
      }
    }
    alert(lines.join('\n'));
  } catch (e) {
    alert('Failed to get monthly status: ' + e);
  }
};

export async function runMonthlyBackup() {
  if (!confirm('Run monthly backup copy now? This will copy the first 2 backups per job for the current month into the monthly folder.')) {
    return;
  }
  
  try {
    await invoke('run_monthly_archive');
    alert('Monthly archive completed');
  } catch (e) {
    alert('Failed to run monthly archive: ' + e);
  }
};

export function collectJobData() {
  const rootDir = document.getElementById('rootDir').value.trim();
  const destinationDir = document.getElementById('destinationDir').value.trim();
  const isMinecraft = state.currentJobType === 'minecraft';
  const isPalworld = state.currentJobType === 'palworld';
  const cluster = isMinecraft
    ? (document.getElementById('monthlyClusterMinecraft')?.value || '')
    : isPalworld
      ? (document.getElementById('monthlyClusterPalworld')?.value || '')
      : (document.getElementById('monthlyCluster')?.value || '');

  if (!rootDir) {
    showError('rootDirError', 'Server root directory is required');
    return null;
  }
  if (!destinationDir) {
    showError('destinationDirError', 'Destination directory is required');
    return null;
  }
  if (!cluster) {
    const clusterErrorId = isMinecraft
      ? 'monthlyClusterMinecraftError'
      : isPalworld
        ? 'monthlyClusterPalworldError'
        : 'monthlyClusterError';
    showError(clusterErrorId, 'Monthly cluster is required');
    return null;
  }

  let name, intervalValue, intervalUnit, map, includeSaves, includeMap, includeServerFiles, includePluginConfigs, retentionDays;

  if (isMinecraft) {
    name = document.getElementById('jobNameMinecraft').value.trim();
    if (!name) {
      showError('jobNameMinecraftError', 'Job name is required');
      return null;
    }
    const rconHost = document.getElementById('rconHostMinecraft').value.trim();
    const rconPortVal = document.getElementById('rconPortMinecraft').value.trim();
    const rconPort = parseInt(rconPortVal, 10);
    const rconPassword = document.getElementById('rconPasswordMinecraft').value;
    if (!rconHost) {
      showError('jobNameMinecraftError', 'RCON host is required');
      return null;
    }
    if (!rconPortVal || isNaN(rconPort) || rconPort < 1 || rconPort > 65535) {
      showError('jobNameMinecraftError', 'RCON port must be 1-65535');
      return null;
    }
    if (!rconPassword) {
      showError('jobNameMinecraftError', 'RCON password is required');
      return null;
    }
    intervalValue = parseInt(document.getElementById('intervalValueMinecraft').value) || 1;
    intervalUnit = document.getElementById('intervalUnitMinecraft').value;
    retentionDays = 30;
    map = '';
    includeSaves = false;
    includeMap = false;
    includeServerFiles = false;
    includePluginConfigs = false;
  } else if (isPalworld) {
    name = document.getElementById('jobNamePalworld').value.trim();
    if (!name) {
      showError('jobNamePalworldError', 'Job name is required');
      return null;
    }
    intervalValue = parseInt(document.getElementById('intervalValuePalworld').value) || 1;
    intervalUnit = document.getElementById('intervalUnitPalworld').value;
    retentionDays = parseInt(document.getElementById('retentionDaysPalworld').value) || 7;
    map = '';
    includeSaves = false;
    includeMap = false;
    includeServerFiles = false;
    includePluginConfigs = false;
  } else {
    map = document.getElementById('mapSelect').value;
    name = document.getElementById('jobName').value.trim();
    if (!map) {
      showError('mapError', 'Map selection is required');
      return null;
    }
    if (!name) {
      showError('jobNameError', 'Job name is required');
      return null;
    }
    intervalValue = parseInt(document.getElementById('intervalValue').value) || 1;
    intervalUnit = document.getElementById('intervalUnit').value;
    retentionDays = parseInt(document.getElementById('retentionDays').value) || 7;
    includeSaves = document.getElementById('includeSaves').checked;
    includeMap = document.getElementById('includeMap').checked;
    includeServerFiles = document.getElementById('includeServerFiles').checked;
    includePluginConfigs = document.getElementById('includePluginConfigs').checked;
  }

  clearAllErrors();

  const payload = {
    job_type: state.currentJobType,
    monthly_cluster: cluster,
    name,
    root_dir: rootDir,
    destination_dir: destinationDir,
    map,
    include_saves: includeSaves,
    include_map: includeMap,
    include_server_files: includeServerFiles,
    include_plugin_configs: includePluginConfigs,
    interval_value: intervalValue,
    interval_unit: intervalUnit,
    retention_days: retentionDays,
    enabled: document.getElementById('enabled').checked
  };
  if (isMinecraft) {
    payload.rcon_host = document.getElementById('rconHostMinecraft').value.trim();
    payload.rcon_port = parseInt(document.getElementById('rconPortMinecraft').value, 10) || 25575;
    payload.rcon_password = document.getElementById('rconPasswordMinecraft').value;
  } else if (isPalworld) {
    const apiHost = document.getElementById('apiHostPalworld').value.trim();
    payload.rcon_host = apiHost || null;
    payload.rcon_port = null;
    payload.rcon_password = null;
  }
  return payload;
}

export function setJobFormVisibilityForType(jobType) {
  const type = jobType || 'ark';
  const arkEl = document.getElementById('jobFormArkFields');
  const mcEl = document.getElementById('jobFormMinecraftFields');
  const pwEl = document.getElementById('jobFormPalworldFields');
  if (arkEl) arkEl.style.display = type === 'ark' ? '' : 'none';
  if (mcEl) mcEl.style.display = type === 'minecraft' ? '' : 'none';
  if (pwEl) pwEl.style.display = type === 'palworld' ? '' : 'none';
}

export async function loadJobIntoForm(job) {
  state.currentJobId = job.id;
  state.currentJobType = job.job_type || 'ark';
  const isMinecraft = state.currentJobType === 'minecraft';
  const isPalworld = state.currentJobType === 'palworld';

  document.getElementById('rootDir').value = job.root_dir || '';
  document.getElementById('destinationDir').value = job.destination_dir || '';
  document.getElementById('enabled').checked = job.enabled || false;

  if (isMinecraft) {
    document.getElementById('jobNameMinecraft').value = job.name || '';
    const mcCluster = document.getElementById('monthlyClusterMinecraft');
    if (mcCluster) mcCluster.value = job.monthly_cluster || 'Minecraft';
    document.getElementById('intervalValueMinecraft').value = job.interval_value || 1;
    document.getElementById('intervalUnitMinecraft').value = job.interval_unit || 'minutes';
    document.getElementById('rconHostMinecraft').value = job.rcon_host || '';
    document.getElementById('rconPortMinecraft').value = job.rcon_port || 25575;
    document.getElementById('rconPasswordMinecraft').value = job.rcon_password || '';
  } else if (isPalworld) {
    document.getElementById('jobNamePalworld').value = job.name || '';
    const pwCluster = document.getElementById('monthlyClusterPalworld');
    if (pwCluster) pwCluster.value = job.monthly_cluster || 'Palworld';
    document.getElementById('intervalValuePalworld').value = job.interval_value || 1;
    document.getElementById('intervalUnitPalworld').value = job.interval_unit || 'minutes';
    document.getElementById('retentionDaysPalworld').value = job.retention_days || 7;
    document.getElementById('apiHostPalworld').value = job.rcon_host || '';
  } else {
    await refreshMapSelects(true, { mapSelect: job.map || '' });
    document.getElementById('jobName').value = job.name || '';
    const arkCluster = document.getElementById('monthlyCluster');
    if (arkCluster) arkCluster.value = job.monthly_cluster || 'ASA Legacy';
    document.getElementById('includeSaves').checked = job.include_saves || false;
    document.getElementById('includeMap').checked = job.include_map || false;
    document.getElementById('includeServerFiles').checked = job.include_server_files || false;
    document.getElementById('includePluginConfigs').checked = job.include_plugin_configs || false;
    document.getElementById('intervalValue').value = job.interval_value || 1;
    document.getElementById('intervalUnit').value = job.interval_unit || 'minutes';
    document.getElementById('retentionDays').value = job.retention_days || 7;
  }

  setJobFormVisibilityForType(state.currentJobType);
  clearAllErrors();
}

export function showError(elementId, message) {
  document.getElementById(elementId).textContent = message;
}

export function clearError(elementId) {
  document.getElementById(elementId).textContent = '';
}

export function clearAllErrors() {
  clearError('rootDirError');
  clearError('destinationDirError');
  clearError('mapError');
  clearError('jobNameError');
  const clErr = document.getElementById('monthlyClusterError');
  if (clErr) clErr.textContent = '';
  const mcErr = document.getElementById('jobNameMinecraftError');
  if (mcErr) mcErr.textContent = '';
  const mcClErr = document.getElementById('monthlyClusterMinecraftError');
  if (mcClErr) mcClErr.textContent = '';
}
