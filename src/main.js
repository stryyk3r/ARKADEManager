import './style.css';
import { invoke } from '@tauri-apps/api/core';
import { open, confirm } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';

let currentJobId = null;
let currentJobType = 'ark'; // 'ark' | 'minecraft' - used when editing
let jobsRefreshInterval = null;
let statusUpdateInterval = null;
let logsRefreshInterval = null;
let updateCheckInterval = null;
let allJobs = [];
let currentRunningJob = null;
const GITHUB_RELEASES_URL = 'https://github.com/stryyk3r/ARKADEManager/releases';

// Initialize app
document.addEventListener('DOMContentLoaded', async () => {
  // Load theme (default dark to match server manager style)
  try {
    const config = await invoke('get_config');
    const theme = config.theme || 'dark';
    document.documentElement.setAttribute('data-theme', theme);
    updateThemeIcon(theme);
  } catch (e) {
    document.documentElement.setAttribute('data-theme', 'dark');
    updateThemeIcon('dark');
  }
  
  // Setup tab switching
  document.querySelectorAll('.nav-item').forEach(tab => {
    tab.addEventListener('click', () => {
      const tabName = tab.dataset.tab;
      switchTab(tabName);
    });
  });
  
  // Theme toggle
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

  // Job search & filter
  const jobSearchInput = document.getElementById('jobSearchInput');
  const jobStatusFilter = document.getElementById('jobStatusFilter');
  const jobTypeFilter = document.getElementById('jobTypeFilter');
  if (jobSearchInput) jobSearchInput.addEventListener('input', applyJobFilters);
  if (jobStatusFilter) jobStatusFilter.addEventListener('change', applyJobFilters);
  if (jobTypeFilter) jobTypeFilter.addEventListener('change', applyJobFilters);
  
  // Load initial jobs
  await refreshJobs();
  
  // Start auto-refresh
  jobsRefreshInterval = setInterval(refreshJobs, 30000); // 30 seconds
  statusUpdateInterval = setInterval(updateStatus, 2000); // 2 seconds
  logsRefreshInterval = setInterval(refreshLogs, 5000); // 5 seconds
  
  // Listen for Tauri events
  await listen('status_update', (event) => {
    updateStatusFromEvent(event.payload);
  });
  
  await listen('job_updated', (event) => {
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

  // Initial status and logs
  await updateStatus();
  await refreshLogs();
  
  // Load and display version
  await loadVersion();
  
  // Check for updates on startup and then every hour
  await checkForUpdates();
  updateCheckInterval = setInterval(checkForUpdates, 3600000); // 1 hour
  
  // Setup update box click handler
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
  
  // Initialize Plugin Toggle tab
  await loadPluginToggleServers();
  initAdminSelect(document.getElementById('pluginToggleServerSelect'));

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
  
  // Ensure job form is hidden on load
  const jobFormContainer = document.getElementById('jobFormContainer');
  if (jobFormContainer) {
    jobFormContainer.style.display = 'none';
    jobFormContainer.style.visibility = 'hidden';
    jobFormContainer.style.height = '0';
    jobFormContainer.style.overflow = 'hidden';
    jobFormContainer.classList.remove('show');
  }
  
  // Close modal when clicking outside
  const modal = document.getElementById('addJobModal');
  if (modal) {
    modal.addEventListener('click', (e) => {
      if (e.target === modal) {
        closeAddJobModal();
      }
    });
  }
});

function switchTab(tabName) {
  document.querySelectorAll('.nav-item').forEach(t => {
    t.classList.remove('active');
  });
  document.querySelectorAll('.tab-pane').forEach(p => p.classList.remove('active'));

  const activeTab = document.querySelector(`.nav-item[data-tab="${tabName}"]`);
  if (activeTab) activeTab.classList.add('active');
  document.getElementById(tabName).classList.add('active');
  
  // Load plugin destinations when new plugins tab is opened
  if (tabName === 'new-plugins' && typeof window.refreshPluginDestinations === 'function') {
    window.refreshPluginDestinations();
  }
  
  // Refresh server list when Plugin Toggle tab is opened
  if (tabName === 'plugin-toggle') {
    loadPluginToggleServers();
  }
}

window.pickRootDir = async () => {
  try {
    console.log('Opening directory picker...');
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Server Root Directory'
    });
    console.log('Selected:', selected);
    if (selected) {
      document.getElementById('rootDir').value = selected;
      clearError('rootDirError');
    } else {
      console.log('No directory selected');
    }
  } catch (e) {
    console.error('Failed to pick directory:', e);
    alert('Failed to open directory picker: ' + e.message);
  }
};

window.pickDestinationDir = async () => {
  try {
    console.log('Opening directory picker...');
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Destination Directory'
    });
    console.log('Selected:', selected);
    if (selected) {
      document.getElementById('destinationDir').value = selected;
      clearError('destinationDirError');
    } else {
      console.log('No directory selected');
    }
  } catch (e) {
    console.error('Failed to pick directory:', e);
    alert('Failed to open directory picker: ' + e.message);
  }
};

let currentWizardStep = 1;
let backupType = null; // 'ark' | 'minecraft' - set when user completes step 1

function getTotalWizardSteps() {
  return backupType === 'minecraft' ? 4 : 5;
}

window.showAddJobForm = () => {
  currentJobId = null;
  currentWizardStep = 1;
  backupType = null;
  resetWizard();
  document.getElementById('addJobModal').classList.add('show');
  updateWizardStep();
};

window.closeAddJobModal = () => {
  document.getElementById('addJobModal').classList.remove('show');
  resetWizard();
};

function resetWizard() {
  currentWizardStep = 1;
  backupType = null;
  document.getElementById('wizardBackupTypeArk').checked = false;
  document.getElementById('wizardBackupTypeMinecraft').checked = false;
  document.getElementById('wizardRootDir').value = '';
  document.getElementById('wizardDestinationDir').value = '';
  document.getElementById('wizardMapSelect').value = '';
  document.getElementById('wizardJobName').value = '';
  document.getElementById('wizardIncludeSaves').checked = false;
  document.getElementById('wizardIncludeMap').checked = false;
  document.getElementById('wizardIncludeServerFiles').checked = false;
  document.getElementById('wizardIncludePluginConfigs').checked = false;
  document.getElementById('wizardIntervalValue').value = '1';
  document.getElementById('wizardIntervalUnit').value = 'minutes';
  document.getElementById('wizardRetentionDays').value = '7';
  document.getElementById('wizardEnabled').checked = false;
  const mcName = document.getElementById('wizardMinecraftJobName');
  const mcInterval = document.getElementById('wizardMinecraftIntervalValue');
  const mcUnit = document.getElementById('wizardMinecraftIntervalUnit');
  const mcEnabled = document.getElementById('wizardMinecraftEnabled');
  const mcRconHost = document.getElementById('wizardMinecraftRconHost');
  const mcRconPort = document.getElementById('wizardMinecraftRconPort');
  const mcRconPassword = document.getElementById('wizardMinecraftRconPassword');
  if (mcName) mcName.value = '';
  if (mcInterval) mcInterval.value = '1';
  if (mcUnit) mcUnit.value = 'minutes';
  if (mcEnabled) mcEnabled.checked = false;
  if (mcRconHost) mcRconHost.value = '';
  if (mcRconPort) mcRconPort.value = '25575';
  if (mcRconPassword) mcRconPassword.value = '';
  clearWizardErrors();
}

function clearWizardErrors() {
  const backupTypeEl = document.getElementById('wizardBackupTypeError');
  if (backupTypeEl) backupTypeEl.textContent = '';
  document.getElementById('wizardRootDirError').textContent = '';
  document.getElementById('wizardDestinationDirError').textContent = '';
  document.getElementById('wizardMapError').textContent = '';
  document.getElementById('wizardJobNameError').textContent = '';
  const arkClusterErr = document.getElementById('wizardMonthlyClusterArkError');
  if (arkClusterErr) arkClusterErr.textContent = '';
  const mcErr = document.getElementById('wizardMinecraftJobNameError');
  if (mcErr) mcErr.textContent = '';
  const mcClusterErr = document.getElementById('wizardMonthlyClusterMinecraftError');
  if (mcClusterErr) mcClusterErr.textContent = '';
  const rconHostErr = document.getElementById('wizardMinecraftRconHostError');
  if (rconHostErr) rconHostErr.textContent = '';
  const rconPortErr = document.getElementById('wizardMinecraftRconPortError');
  if (rconPortErr) rconPortErr.textContent = '';
  const rconPwErr = document.getElementById('wizardMinecraftRconPasswordError');
  if (rconPwErr) rconPwErr.textContent = '';
}

function updateWizardStep() {
  const totalSteps = getTotalWizardSteps();
  const stepIds = ['wizardStep1', 'wizardStep2', 'wizardStep3', 'wizardStep4', 'wizardStep5', 'wizardStepMinecraftConfig'];
  stepIds.forEach(id => {
    const el = document.getElementById(id);
    if (el) el.classList.remove('active');
  });

  // Show current step panel
  if (currentWizardStep === 1) {
    document.getElementById('wizardStep1').classList.add('active');
  } else if (backupType === 'minecraft' && currentWizardStep === 4) {
    document.getElementById('wizardStepMinecraftConfig').classList.add('active');
  } else {
    document.getElementById(`wizardStep${currentWizardStep}`).classList.add('active');
  }

  // Update step indicators (show 2 dots for Minecraft, 5 for Ark / step 1)
  for (let i = 1; i <= 5; i++) {
    const dot = document.getElementById(`step${i}Dot`);
    const connector = document.getElementById(`connector${i}`);
    const visible = i <= totalSteps;
    if (dot) {
      dot.style.display = visible ? '' : 'none';
      dot.classList.remove('active', 'completed');
      if (i < currentWizardStep) dot.classList.add('completed');
      else if (i === currentWizardStep) dot.classList.add('active');
    }
    if (connector) {
      connector.style.display = visible && i < totalSteps ? '' : 'none';
      connector.classList.toggle('completed', i < currentWizardStep);
    }
  }

  // Update buttons
  document.getElementById('wizardPrevBtn').style.display = currentWizardStep > 1 ? 'block' : 'none';
  document.getElementById('wizardNextBtn').style.display = currentWizardStep < totalSteps ? 'block' : 'none';
  document.getElementById('wizardFinishBtn').style.display = currentWizardStep === totalSteps ? 'block' : 'none';
}

window.wizardNextStep = () => {
  if (validateCurrentStep()) {
    const totalSteps = getTotalWizardSteps();
    if (currentWizardStep < totalSteps) {
      currentWizardStep++;
      updateWizardStep();
    }
  }
};

window.wizardPreviousStep = () => {
  if (currentWizardStep > 1) {
    currentWizardStep--;
    if (currentWizardStep === 1) backupType = null;
    updateWizardStep();
  }
};

function validateCurrentStep() {
  clearWizardErrors();
  let isValid = true;

  switch (currentWizardStep) {
    case 1: {
      const arkChecked = document.getElementById('wizardBackupTypeArk').checked;
      const minecraftChecked = document.getElementById('wizardBackupTypeMinecraft').checked;
      if (!arkChecked && !minecraftChecked) {
        document.getElementById('wizardBackupTypeError').textContent = 'Please choose ARK or Minecraft';
        isValid = false;
      } else {
        backupType = minecraftChecked ? 'minecraft' : 'ark';
      }
      break;
    }
    case 2:
      if (backupType === 'ark') {
        const rootDir = document.getElementById('wizardRootDir').value.trim();
        if (!rootDir) {
          document.getElementById('wizardRootDirError').textContent = 'Server root directory is required';
          isValid = false;
        }
      }
      break;
    case 3: {
      const destDir = document.getElementById('wizardDestinationDir').value.trim();
      if (!destDir) {
        document.getElementById('wizardDestinationDirError').textContent = 'Destination directory is required';
        isValid = false;
      }
      break;
    }
    case 4:
      if (backupType === 'minecraft') {
        const mcJobName = document.getElementById('wizardMinecraftJobName').value.trim();
        if (!mcJobName) {
          document.getElementById('wizardMinecraftJobNameError').textContent = 'Job name is required';
          isValid = false;
        }
        const mcCluster = document.getElementById('wizardMonthlyClusterMinecraft')?.value;
        if (!mcCluster) {
          document.getElementById('wizardMonthlyClusterMinecraftError').textContent = 'Monthly cluster is required';
          isValid = false;
        }
        const rconHost = document.getElementById('wizardMinecraftRconHost').value.trim();
        if (!rconHost) {
          document.getElementById('wizardMinecraftRconHostError').textContent = 'RCON host is required';
          isValid = false;
        }
        const rconPortVal = document.getElementById('wizardMinecraftRconPort').value.trim();
        const rconPort = parseInt(rconPortVal, 10);
        if (!rconPortVal || isNaN(rconPort) || rconPort < 1 || rconPort > 65535) {
          document.getElementById('wizardMinecraftRconPortError').textContent = 'RCON port must be 1-65535';
          isValid = false;
        }
        const rconPassword = document.getElementById('wizardMinecraftRconPassword').value;
        if (!rconPassword) {
          document.getElementById('wizardMinecraftRconPasswordError').textContent = 'RCON password is required';
          isValid = false;
        }
      } else {
        const map = document.getElementById('wizardMapSelect').value;
        if (!map) {
          document.getElementById('wizardMapError').textContent = 'Map selection is required';
          isValid = false;
        }
      }
      break;
    case 5: {
      const jobName = document.getElementById('wizardJobName').value.trim();
      if (!jobName) {
        document.getElementById('wizardJobNameError').textContent = 'Job name is required';
        isValid = false;
      }
      const arkCluster = document.getElementById('wizardMonthlyClusterArk')?.value;
      if (!arkCluster) {
        document.getElementById('wizardMonthlyClusterArkError').textContent = 'Monthly cluster is required';
        isValid = false;
      }
      break;
    }
  }

  return isValid;
}

window.pickWizardRootDir = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Server Root Directory'
    });
    if (selected) {
      document.getElementById('wizardRootDir').value = selected;
      document.getElementById('wizardRootDirError').textContent = '';
    }
  } catch (e) {
    console.error('Failed to pick directory:', e);
    alert('Failed to open directory picker: ' + e.message);
  }
};

window.pickWizardDestinationDir = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Destination Directory'
    });
    if (selected) {
      document.getElementById('wizardDestinationDir').value = selected;
      document.getElementById('wizardDestinationDirError').textContent = '';
    }
  } catch (e) {
    console.error('Failed to pick directory:', e);
    alert('Failed to open directory picker: ' + e.message);
  }
};

window.wizardFinish = async () => {
  if (!validateCurrentStep()) {
    return;
  }

  let job;
  if (backupType === 'minecraft') {
    job = {
      job_type: 'minecraft',
      monthly_cluster: document.getElementById('wizardMonthlyClusterMinecraft').value,
      name: document.getElementById('wizardMinecraftJobName').value.trim(),
      root_dir: document.getElementById('wizardRootDir').value.trim(),
      destination_dir: document.getElementById('wizardDestinationDir').value.trim(),
      map: '',
      include_saves: false,
      include_map: false,
      include_server_files: false,
      include_plugin_configs: false,
      interval_value: parseInt(document.getElementById('wizardMinecraftIntervalValue').value) || 1,
      interval_unit: document.getElementById('wizardMinecraftIntervalUnit').value,
      retention_days: 30,
      enabled: document.getElementById('wizardMinecraftEnabled').checked,
      rcon_host: document.getElementById('wizardMinecraftRconHost').value.trim(),
      rcon_port: parseInt(document.getElementById('wizardMinecraftRconPort').value, 10) || 25575,
      rcon_password: document.getElementById('wizardMinecraftRconPassword').value
    };
  } else {
    job = {
      job_type: 'ark',
      monthly_cluster: document.getElementById('wizardMonthlyClusterArk').value,
      name: document.getElementById('wizardJobName').value.trim(),
      root_dir: document.getElementById('wizardRootDir').value.trim(),
      destination_dir: document.getElementById('wizardDestinationDir').value.trim(),
      map: document.getElementById('wizardMapSelect').value,
      include_saves: document.getElementById('wizardIncludeSaves').checked,
      include_map: document.getElementById('wizardIncludeMap').checked,
      include_server_files: document.getElementById('wizardIncludeServerFiles').checked,
      include_plugin_configs: document.getElementById('wizardIncludePluginConfigs').checked,
      interval_value: parseInt(document.getElementById('wizardIntervalValue').value) || 1,
      interval_unit: document.getElementById('wizardIntervalUnit').value,
      retention_days: parseInt(document.getElementById('wizardRetentionDays').value) || 7,
      enabled: document.getElementById('wizardEnabled').checked
    };
  }

  try {
    await invoke('add_job', { job });
    closeAddJobModal();
    await refreshJobs();
  } catch (e) {
    alert('Failed to add job: ' + e);
  }
};

window.cancelJobForm = () => {
  const formContainer = document.getElementById('jobFormContainer');
  formContainer.style.display = 'none';
  formContainer.style.visibility = 'hidden';
  formContainer.style.height = '0';
  formContainer.style.overflow = 'hidden';
  formContainer.classList.remove('show');
  clearForm();
  currentJobId = null;
};

window.addJob = async () => {
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

window.updateJob = async (jobId) => {
  if (!jobId) {
    jobId = currentJobId;
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
    
    loadJobIntoForm(job);
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
        
        jobData.id = currentJobId;
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

window.deleteJob = async (jobId) => {
  if (!jobId) {
    jobId = currentJobId;
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
    if (currentJobId === jobId) {
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

window.runJobNow = async (jobId) => {
  if (!jobId) {
    jobId = currentJobId;
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

window.openBackupLocation = async (destinationDir) => {
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

window.openLogsFolder = async () => {
  try {
    await invoke('open_logs_folder');
  } catch (e) {
    alert('Failed to open logs folder: ' + e);
  }
};

window.clearForm = () => {
  document.getElementById('rootDir').value = '';
  document.getElementById('destinationDir').value = '';
  document.getElementById('mapSelect').value = '';
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

function showBackupProgress(jobName, percent) {
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

function hideBackupProgress() {
  const card = document.getElementById('backupProgressCard');
  if (card) card.classList.remove('show');
}

function updateThemeIcon(theme) {
  const icon = document.getElementById('themeToggleIcon');
  if (icon) icon.textContent = theme === 'dark' ? '☀' : '☾';
}

const JOB_GROUP_ORDER = ['asa', 'ase', 'minecraft', 'palworld'];

const JOB_GROUP_META = {
  asa: { label: 'ARK: Survival Ascended', letter: 'A', color: 'teal' },
  ase: { label: 'ARK: Survival Evolved', letter: 'A', color: 'yellow' },
  minecraft: { label: 'Minecraft', letter: 'M', color: 'green' },
  palworld: { label: 'Palworld', letter: 'P', color: 'blue' },
};

function getJobGroupKey(job) {
  if (job.job_type === 'minecraft') return 'minecraft';
  const cluster = job.monthly_cluster || '';
  if (cluster === 'ASE Legacy') return 'ase';
  if (cluster === 'Palworld') return 'palworld';
  return 'asa';
}

function computeSuccessRate(jobs) {
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

function computeTotalStorage(jobs) {
  const total = (jobs || []).reduce((sum, j) => sum + (j.last_file_size || 0), 0);
  return formatFileSize(total);
}

function getJobRowState(job) {
  if (currentRunningJob && currentRunningJob === job.name) {
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

function jobMatchesFilter(job) {
  const search = (document.getElementById('jobSearchInput')?.value || '').toLowerCase().trim();
  const statusFilter = document.getElementById('jobStatusFilter')?.value || 'all';
  const typeFilter = document.getElementById('jobTypeFilter')?.value || 'all';

  if (typeFilter !== 'all' && (job.job_type || 'ark') !== typeFilter) return false;

  if (statusFilter === 'enabled' && !job.enabled) return false;
  if (statusFilter === 'disabled' && job.enabled) return false;
  if (statusFilter === 'error' && !job.last_error) return false;

  if (search) {
    const haystack = [
      job.name,
      job.map,
      job.monthly_cluster,
      job.root_dir,
      job.destination_dir,
      job.job_type || 'ark'
    ].join(' ').toLowerCase();
    if (!haystack.includes(search)) return false;
  }
  return true;
}

function applyJobFilters() {
  document.querySelectorAll('#jobsTableBody tr[data-job-id]').forEach(row => {
    const jobId = row.dataset.jobId;
    const job = allJobs.find(j => j.id === jobId);
    row.classList.toggle('hidden-row', job ? !jobMatchesFilter(job) : false);
  });

  document.querySelectorAll('#jobsTableBody tr.group-header-row').forEach(groupRow => {
    const groupKey = groupRow.dataset.groupKey;
    const jobRows = [...document.querySelectorAll(`#jobsTableBody tr[data-job-id][data-group="${groupKey}"]`)]
      .filter(r => !r.classList.contains('hidden-row'));
    groupRow.classList.toggle('hidden-row', jobRows.length === 0);
  });

  const jobsPanel = document.getElementById('jobsPanel');
  const visibleJobs = document.querySelectorAll('#jobsTableBody tr[data-job-id]:not(.hidden-row)').length;
  if (jobsPanel) jobsPanel.style.display = allJobs.length > 0 ? '' : 'none';
}

function updateJobCounts(jobs) {
  const list = jobs || [];
  const configured = document.getElementById('configuredJobsBadge');
  if (configured) configured.textContent = `${list.length} configured`;

  const enabled = list.filter(j => j.enabled).length;
  const saving = currentRunningJob ? 1 : 0;

  const activeValue = document.getElementById('statActiveValue');
  const activeMeta = document.getElementById('statActiveMeta');
  if (activeValue) activeValue.textContent = String(saving);
  if (activeMeta) activeMeta.textContent = saving === 1 ? '1 saving' : `${saving} saving`;

  const successValue = document.getElementById('statSuccessValue');
  if (successValue) successValue.textContent = computeSuccessRate(list);

  const storageValue = document.getElementById('statStorageValue');
  if (storageValue) storageValue.textContent = computeTotalStorage(list);
}

window.refreshJobs = async () => {
  try {
    const jobs = await invoke('list_jobs');
    renderJobsTable(jobs);
  } catch (e) {
    console.error('Failed to refresh jobs:', e);
  }
};

window.showMonthlyStatus = async () => {
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

window.runMonthlyBackup = async () => {
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

window.refreshLogs = async () => {
  try {
    const logs = await invoke('read_logs', { lines: 100 });
    document.getElementById('logsContent').textContent = logs;
  } catch (e) {
    console.error('Failed to refresh logs:', e);
  }
};

window.clearLogsView = () => {
  document.getElementById('logsContent').textContent = '';
};

function collectJobData() {
  const rootDir = document.getElementById('rootDir').value.trim();
  const destinationDir = document.getElementById('destinationDir').value.trim();
  const isMinecraft = currentJobType === 'minecraft';
  const cluster = isMinecraft
    ? (document.getElementById('monthlyClusterMinecraft')?.value || '')
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
    showError(isMinecraft ? 'monthlyClusterMinecraftError' : 'monthlyClusterError', 'Monthly cluster is required');
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
    job_type: currentJobType,
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
  }
  return payload;
}

function renderJobsTable(jobs) {
  allJobs = jobs || [];
  const emptyAll = document.getElementById('backupEmptyAll');
  const toolbar = document.getElementById('backupToolbar');
  const jobsPanel = document.getElementById('jobsPanel');
  const tbody = document.getElementById('jobsTableBody');
  const hasAny = allJobs.length > 0;

  if (emptyAll) emptyAll.style.display = hasAny ? 'none' : 'block';
  if (toolbar) toolbar.style.display = hasAny ? '' : 'none';
  if (jobsPanel) jobsPanel.style.display = hasAny ? '' : 'none';

  updateJobCounts(allJobs);
  if (!tbody) return;
  tbody.innerHTML = '';

  const grouped = {};
  for (const job of allJobs) {
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

  if (!window._jobMenuListenerAttached) {
    document.addEventListener('click', (e) => {
      if (!e.target.closest('.job-menu')) {
        document.querySelectorAll('.job-menu-dropdown').forEach(d => d.classList.remove('show'));
      }
    });
    window._jobMenuListenerAttached = true;
  }
}

function appendUnifiedJobRow(tbody, job, groupKey) {
  const row = tbody.insertRow();
  row.dataset.jobId = job.id;
  row.dataset.group = groupKey;
  const state = getJobRowState(job);

  const nameCell = row.insertCell();
  nameCell.innerHTML = `
    <div class="job-row-name">
      <span class="job-status-dot ${state.dot}"></span>
      <div class="job-name-block">
        <span class="job-name-primary ${state.running ? 'running' : ''}">${escapeHtml(job.name)}</span>
        ${state.sub ? `<span class="job-name-sub ${state.running ? 'progress' : ''}">${escapeHtml(state.sub)}</span>` : ''}
      </div>
    </div>`;

  const intervalCell = row.insertCell();
  intervalCell.textContent = `${job.interval_value} ${job.interval_unit}`;
  intervalCell.className = 'cell-metric cell-metric-muted';

  const nextCell = row.insertCell();
  nextCell.textContent = job.next_run_at ? new Date(job.next_run_at).toLocaleString() : 'N/A';

  const lastCell = row.insertCell();
  if (job.last_error && !state.running) {
    lastCell.innerHTML = state.sub === 'warning'
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
    document.querySelectorAll('.job-menu-dropdown').forEach(d => d.classList.remove('show'));
    menuContainer.querySelector('.job-menu-dropdown').classList.toggle('show');
  };
  const dropdown = document.createElement('div');
  dropdown.className = 'job-menu-dropdown';

  const runItem = document.createElement('button');
  runItem.className = 'job-menu-item';
  runItem.type = 'button';
  runItem.textContent = 'Run Now';
  runItem.onclick = (e) => { e.stopPropagation(); dropdown.classList.remove('show'); runJobNow(job.id); };

  const backupLocationItem = document.createElement('button');
  backupLocationItem.className = 'job-menu-item';
  backupLocationItem.type = 'button';
  backupLocationItem.textContent = 'Backup Location';
  backupLocationItem.onclick = (e) => {
    e.stopPropagation();
    dropdown.classList.remove('show');
    openBackupLocation(job.destination_dir);
  };

  const editItem = document.createElement('button');
  editItem.className = 'job-menu-item';
  editItem.type = 'button';
  editItem.textContent = 'Edit';
  editItem.onclick = (e) => {
    e.stopPropagation();
    dropdown.classList.remove('show');
    updateJob(job.id);
  };

  const deleteItem = document.createElement('button');
  deleteItem.className = 'job-menu-item danger';
  deleteItem.type = 'button';
  deleteItem.textContent = 'Delete';
  deleteItem.onclick = (e) => {
    e.stopPropagation();
    dropdown.classList.remove('show');
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

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str || '';
  return div.innerHTML;
}

function setJobFormVisibilityForType(jobType) {
  const isMinecraft = (jobType || 'ark') === 'minecraft';
  const arkEl = document.getElementById('jobFormArkFields');
  const mcEl = document.getElementById('jobFormMinecraftFields');
  if (arkEl) arkEl.style.display = isMinecraft ? 'none' : '';
  if (mcEl) mcEl.style.display = isMinecraft ? '' : 'none';
}

function loadJobIntoForm(job) {
  currentJobId = job.id;
  currentJobType = job.job_type || 'ark';
  const isMinecraft = currentJobType === 'minecraft';

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
  } else {
    document.getElementById('mapSelect').value = job.map || '';
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

  setJobFormVisibilityForType(currentJobType);
  clearAllErrors();
}

async function updateStatus() {
  try {
    const status = await invoke('get_status');
    updateStatusFromEvent(status);
  } catch (e) {
    console.error('Failed to update status:', e);
  }
}

function resolveCurrentJobName(currentJobId) {
  if (!currentJobId) return null;
  const job = allJobs.find(j => j.id === currentJobId);
  return job ? job.name : null;
}

function formatCurrentJobLabel(currentJobId) {
  return resolveCurrentJobName(currentJobId) || 'None';
}

function updateStatusFromEvent(status) {
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

  currentRunningJob = resolveCurrentJobName(status.current_job);

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

  updateJobCounts(allJobs);
  refreshJobRowStates();
}

function refreshJobRowStates() {
  document.querySelectorAll('#jobsTableBody tr[data-job-id]').forEach(row => {
    const job = allJobs.find(j => j.id === row.dataset.jobId);
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

function formatFileSize(bytes) {
  if (!bytes) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
}

function showError(elementId, message) {
  document.getElementById(elementId).textContent = message;
}

function clearError(elementId) {
  document.getElementById(elementId).textContent = '';
}

function clearAllErrors() {
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

// Plugin Manager Functions
let pluginSourcePath = null;
let pluginSourcePlugins = [];
let pluginDestinations = [];
let pendingInstallation = null; // Store installation data for confirmation

window.browsePluginSource = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Source Folder (containing plugin subdirectories)'
    });
    
    if (selected) {
      pluginSourcePath = selected;
      document.getElementById('pluginSourcePath').textContent = `Source: ${selected}`;
      await loadSourcePlugins(selected);
    }
  } catch (error) {
    console.error('Error browsing source folder:', error);
    alert('Failed to browse source folder: ' + error);
  }
};

async function loadSourcePlugins(sourcePath) {
  try {
    const plugins = await invoke('list_source_plugins', { sourcePath });
    pluginSourcePlugins = plugins;
    renderSourcePlugins();
    updateInstallButtonState();
  } catch (error) {
    console.error('Error loading source plugins:', error);
    document.getElementById('pluginSourceList').innerHTML = 
      `<div class="empty-state" style="color: var(--error);">Error: ${error}</div>`;
  }
}

function renderSourcePlugins() {
  const container = document.getElementById('pluginSourceList');
  const toggleBtn = document.getElementById('sourceToggleAllBtn');

  if (pluginSourcePlugins.length === 0) {
    container.innerHTML = '<div class="empty-state">No plugin folders found in source directory</div>';
    if (toggleBtn) toggleBtn.disabled = true;
    return;
  }

  container.innerHTML = pluginSourcePlugins.map((plugin, index) => `
    <div class="plugin-item">
      <input type="checkbox" id="source-plugin-${index}" data-path="${plugin.path}">
      <label for="source-plugin-${index}" class="plugin-item-label">${plugin.name}</label>
    </div>
  `).join('');

  container.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
    checkbox.addEventListener('change', () => {
      updateInstallButtonState();
      updatePluginToggleAllLabels();
    });
  });

  if (toggleBtn) toggleBtn.disabled = false;
  updatePluginToggleAllLabels();
}

window.refreshPluginDestinations = async () => {
  try {
    const destinations = await invoke('discover_plugin_destinations');
    pluginDestinations = destinations;
    renderDestinations();
    updateInstallButtonState();
  } catch (error) {
    console.error('Error loading destinations:', error);
    document.getElementById('pluginDestinationList').innerHTML =
      `<div class="empty-state" style="color: var(--error);">Error: ${error}</div>`;
    const toggleBtn = document.getElementById('destToggleAllBtn');
    if (toggleBtn) toggleBtn.disabled = true;
  }
};

function renderDestinations() {
  const container = document.getElementById('pluginDestinationList');
  const toggleBtn = document.getElementById('destToggleAllBtn');

  if (pluginDestinations.length === 0) {
    container.innerHTML = '<div class="empty-state">No ARK servers found. Add backup jobs with a server root, or ensure C:\\arkservers\\asaservers exists.</div>';
    if (toggleBtn) toggleBtn.disabled = true;
    return;
  }

  container.innerHTML = pluginDestinations.map((server, index) => `
    <div class="plugin-item">
      <input type="checkbox" id="dest-server-${index}" data-path="${server.plugin_path}">
      <label for="dest-server-${index}" class="plugin-item-label">${server.name}</label>
    </div>
  `).join('');

  container.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
    checkbox.addEventListener('change', () => {
      updateInstallButtonState();
      updatePluginToggleAllLabels();
    });
  });

  if (toggleBtn) toggleBtn.disabled = false;
  updatePluginToggleAllLabels();
}

function updatePluginToggleAllLabels() {
  const sourceBtn = document.getElementById('sourceToggleAllBtn');
  const destBtn = document.getElementById('destToggleAllBtn');
  const sourceBoxes = document.querySelectorAll('#pluginSourceList input[type="checkbox"]');
  const destBoxes = document.querySelectorAll('#pluginDestinationList input[type="checkbox"]');

  if (sourceBtn && sourceBoxes.length > 0) {
    const allSourceChecked = [...sourceBoxes].every(cb => cb.checked);
    sourceBtn.textContent = allSourceChecked ? 'Deselect All' : 'Select All';
  }

  if (destBtn && destBoxes.length > 0) {
    const allDestChecked = [...destBoxes].every(cb => cb.checked);
    destBtn.textContent = allDestChecked ? 'Deselect All' : 'Select All';
  }
}

window.toggleAllSourcePlugins = () => {
  const checkboxes = document.querySelectorAll('#pluginSourceList input[type="checkbox"]');
  if (checkboxes.length === 0) return;
  const allChecked = [...checkboxes].every(cb => cb.checked);
  checkboxes.forEach(cb => { cb.checked = !allChecked; });
  updatePluginToggleAllLabels();
  updateInstallButtonState();
};

window.toggleAllDestinations = () => {
  const checkboxes = document.querySelectorAll('#pluginDestinationList input[type="checkbox"]');
  if (checkboxes.length === 0) return;
  const allChecked = [...checkboxes].every(cb => cb.checked);
  checkboxes.forEach(cb => { cb.checked = !allChecked; });
  updatePluginToggleAllLabels();
  updateInstallButtonState();
};

function updateInstallButtonState() {
  const sourceSelected = document.querySelectorAll('#pluginSourceList input[type="checkbox"]:checked').length > 0;
  const destSelected = document.querySelectorAll('#pluginDestinationList input[type="checkbox"]:checked').length > 0;
  const installBtn = document.getElementById('installPluginsBtn');
  
  if (installBtn) {
    installBtn.disabled = !(sourceSelected && destSelected);
  }
}

// Make it accessible globally for debugging
window.updateInstallButtonState = updateInstallButtonState;

window.installSelectedPlugins = async () => {
  const sourceCheckboxes = document.querySelectorAll('#pluginSourceList input[type="checkbox"]:checked');
  const destCheckboxes = document.querySelectorAll('#pluginDestinationList input[type="checkbox"]:checked');
  
  if (sourceCheckboxes.length === 0 || destCheckboxes.length === 0) {
    alert('Please select at least one source plugin and one destination server');
    return;
  }
  
  // Get selected plugin and server names for confirmation
  const selectedPlugins = Array.from(sourceCheckboxes).map(cb => {
    const label = document.querySelector(`label[for="${cb.id}"]`);
    return label ? label.textContent : 'Unknown';
  });
  const selectedServers = Array.from(destCheckboxes).map(cb => {
    const label = document.querySelector(`label[for="${cb.id}"]`);
    return label ? label.textContent : 'Unknown';
  });
  
  const sourcePaths = Array.from(sourceCheckboxes).map(cb => cb.dataset.path);
  const destPaths = Array.from(destCheckboxes).map(cb => cb.dataset.path);
  
  // Store installation data for confirmation
  pendingInstallation = {
    sourcePaths,
    destPaths,
    selectedPlugins,
    selectedServers,
    sourceCheckboxes: Array.from(sourceCheckboxes),
    destCheckboxes: Array.from(destCheckboxes)
  };
  
  // Show confirmation modal
  showPluginConfirmModal(selectedPlugins, selectedServers);
};

window.proceedWithPluginInstallation = async () => {
  if (!pendingInstallation) {
    return;
  }
  
  // Copy data before closing modal (closePluginConfirmModal sets pendingInstallation = null)
  const { sourcePaths, destPaths, selectedPlugins, selectedServers, sourceCheckboxes, destCheckboxes } = pendingInstallation;
  pendingInstallation = null;
  const modal = document.getElementById('pluginConfirmModal');
  if (modal) modal.classList.remove('show');
  
  const installBtn = document.getElementById('installPluginsBtn');
  installBtn.disabled = true;
  installBtn.textContent = 'Installing...';
  
  try {
    const result = await invoke('install_plugins', {
      sourcePluginPaths: sourcePaths,
      destinationPluginPaths: destPaths
    });
    
    console.log('Installation complete, result:', result);
    
    // Show results in modal
    console.log('Calling showPluginResults...');
    showPluginResults(result, selectedPlugins, selectedServers);
    console.log('showPluginResults called');
    
    // Clear selections
    sourceCheckboxes.forEach(cb => cb.checked = false);
    destCheckboxes.forEach(cb => cb.checked = false);
    updateInstallButtonState();
    
  } catch (error) {
    console.error('Error installing plugins:', error);
    showPluginResultsError(error);
  } finally {
    if (installBtn) {
      installBtn.disabled = false;
      installBtn.textContent = 'Install Selected Plugins';
    }
  }
};

function showPluginConfirmModal(selectedPlugins, selectedServers) {
  const modal = document.getElementById('pluginConfirmModal');
  const content = document.getElementById('pluginConfirmContent');
  
  let html = '<div style="margin-bottom: 16px;">';
  html += `<p style="margin-bottom: 12px; font-size: 15px;">Are you sure you want to install the following plugins to the selected servers?</p>`;
  html += '</div>';
  
  html += '<div style="margin-bottom: 16px; padding: 12px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 4px;">';
  html += `<strong>Plugins to install (${selectedPlugins.length}):</strong><br>`;
  html += `<div style="margin-top: 8px; margin-left: 12px;">${selectedPlugins.join(', ')}</div>`;
  html += '</div>';
  
  html += '<div style="margin-bottom: 16px; padding: 12px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 4px;">';
  html += `<strong>Destination servers (${selectedServers.length}):</strong><br>`;
  html += `<div style="margin-top: 8px; margin-left: 12px;">${selectedServers.join(', ')}</div>`;
  html += '</div>';
  
  html += '<div style="padding: 12px; background: rgba(255, 193, 7, 0.1); border: 1px solid #ffc107; border-radius: 4px; color: #f57c00;">';
  html += '<strong>⚠ Note:</strong> Files with the same name will be overwritten. Other files in the destination folder are left unchanged.';
  html += '</div>';
  
  content.innerHTML = html;
  modal.classList.add('show');
}

window.closePluginConfirmModal = () => {
  const modal = document.getElementById('pluginConfirmModal');
  modal.classList.remove('show');
  pendingInstallation = null;
};

function showPluginResults(result, selectedPlugins, selectedServers) {
  console.log('showPluginResults called with:', { result, selectedPlugins, selectedServers });
  
  const modal = document.getElementById('pluginResultsModal');
  const title = document.getElementById('pluginResultsTitle');
  const content = document.getElementById('pluginResultsContent');
  
  console.log('Modal elements:', { modal, title, content });
  
  if (!modal) {
    console.error('Results modal element not found!');
    alert('Results modal not found in DOM. Installation completed:\n' + 
          `Files copied: ${result.files_copied}\n` +
          `Files overwritten: ${result.files_overwritten}`);
    return;
  }
  
  if (!title || !content) {
    console.error('Results modal title or content not found!');
    return;
  }
  
  // Ensure confirmation modal is closed
  const confirmModal = document.getElementById('pluginConfirmModal');
  if (confirmModal) {
    confirmModal.classList.remove('show');
  }
  
  title.textContent = 'Installation Complete';
  
  let html = '<div style="margin-bottom: 16px;">';
  html += `<strong>Plugins installed:</strong> ${selectedPlugins.join(', ')}<br>`;
  html += `<strong>Servers updated:</strong> ${selectedServers.join(', ')}<br><br>`;
  html += '</div>';
  
  html += '<div style="margin-bottom: 16px;">';
  html += `<strong>Files copied:</strong> ${result.files_copied}<br>`;
  html += `<strong>Files overwritten:</strong> ${result.files_overwritten}`;
  html += '</div>';
  
  if (result.errors && result.errors.length > 0) {
    html += '<div style="margin-top: 16px; padding: 12px; background: rgba(211, 47, 47, 0.1); border: 1px solid var(--error); border-radius: 4px;">';
    html += '<strong style="color: var(--error);">Errors:</strong><ul style="margin-top: 8px; padding-left: 20px;">';
    result.errors.forEach(error => {
      html += `<li style="color: var(--error); margin-bottom: 4px;">${error}</li>`;
    });
    html += '</ul></div>';
  } else {
    html += '<div style="margin-top: 16px; padding: 12px; background: rgba(56, 142, 60, 0.1); border: 1px solid var(--success); border-radius: 4px; color: var(--success);">';
    html += '<strong>✓ Installation completed successfully!</strong>';
    html += '</div>';
  }
  
  content.innerHTML = html;
  
  // Force show the modal - use both class and inline style to ensure it shows
  modal.classList.add('show');
  modal.style.display = 'flex';
  modal.style.visibility = 'visible';
  modal.style.opacity = '1';
  
  console.log('Modal classes after adding show:', modal.className);
  console.log('Modal style.display:', modal.style.display);
  console.log('Modal computed style:', window.getComputedStyle(modal).display);
  console.log('Modal z-index:', window.getComputedStyle(modal).zIndex);
  
  // Double-check after a brief moment
  setTimeout(() => {
    const computed = window.getComputedStyle(modal);
    console.log('After timeout - Modal display:', computed.display);
    console.log('After timeout - Modal visibility:', computed.visibility);
    if (computed.display === 'none') {
      console.error('Modal is still hidden! Forcing display...');
      modal.style.setProperty('display', 'flex', 'important');
    }
  }, 50);
}

function showPluginResultsError(error) {
  console.log('showPluginResultsError called with:', error);
  
  const modal = document.getElementById('pluginResultsModal');
  const title = document.getElementById('pluginResultsTitle');
  const content = document.getElementById('pluginResultsContent');
  
  if (!modal) {
    console.error('Results modal element not found!');
    alert('Installation failed: ' + error);
    return;
  }
  
  if (!title || !content) {
    console.error('Results modal title or content not found!');
    alert('Installation failed: ' + error);
    return;
  }
  
  // Ensure confirmation modal is closed
  const confirmModal = document.getElementById('pluginConfirmModal');
  if (confirmModal) {
    confirmModal.classList.remove('show');
  }
  
  title.textContent = 'Installation Failed';
  content.innerHTML = `
    <div style="padding: 12px; background: rgba(211, 47, 47, 0.1); border: 1px solid var(--error); border-radius: 4px; color: var(--error);">
      <strong>Error:</strong> ${error}
    </div>
  `;
  
  // Force show the modal
  modal.classList.add('show');
  modal.style.display = 'flex';
  modal.style.visibility = 'visible';
  modal.style.opacity = '1';
  
  console.log('Error modal should be visible now');
}

window.closePluginResultsModal = () => {
  const modal = document.getElementById('pluginResultsModal');
  if (!modal) return;
  modal.classList.remove('show');
  modal.style.display = '';
  modal.style.visibility = '';
  modal.style.opacity = '';
};

// Close modals when clicking outside
document.addEventListener('DOMContentLoaded', () => {
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
});

// Version and Update Functions
function formatReleaseDate(isoDate) {
  if (!isoDate) return '';
  const date = new Date(isoDate);
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

function updateVersionDisplay({ version, isLatest, publishedAt, updateAvailable, updateVersion }) {
  const versionBox = document.getElementById('versionBox');
  const statusBadge = document.getElementById('versionStatusBadge');
  const versionDate = document.getElementById('versionDate');
  const updateBox = document.getElementById('updateBox');

  const displayVersion = version
    ? (version.startsWith('v') ? version : `v${version}`)
    : '';

  if (versionBox && displayVersion) versionBox.textContent = displayVersion;

  const sidebarVersion = document.getElementById('sidebarVersion');
  if (sidebarVersion && displayVersion) sidebarVersion.textContent = displayVersion;

  const settingsVersion = document.getElementById('settingsVersion');
  if (settingsVersion && displayVersion) settingsVersion.textContent = displayVersion;

  const settingsVersionBadge = document.getElementById('settingsVersionBadge');
  const settingsVersionDate = document.getElementById('settingsVersionDate');
  const settingsVersionHint = document.getElementById('settingsVersionHint');
  const settingsUpdateBtn = document.getElementById('settingsUpdateBtn');

  if (statusBadge) {
    if (updateAvailable) {
      statusBadge.textContent = 'Update';
      statusBadge.className = 'version-badge version-badge-update';
      statusBadge.style.display = '';
    } else {
      statusBadge.style.display = 'none';
    }
  }

  if (settingsVersionBadge) {
    if (updateAvailable) {
      settingsVersionBadge.textContent = 'Update';
      settingsVersionBadge.className = 'version-badge version-badge-update';
      settingsVersionBadge.style.display = '';
    } else {
      settingsVersionBadge.style.display = 'none';
    }
  }

  const dateLabel = publishedAt ? `on ${formatReleaseDate(publishedAt)}` : '';
  if (versionDate) versionDate.textContent = dateLabel;
  if (settingsVersionDate) settingsVersionDate.textContent = dateLabel;

  if (settingsVersionHint) {
    settingsVersionHint.textContent = updateAvailable
      ? `v${updateVersion || ''} is available`
      : isLatest
        ? 'You are on the latest release'
        : '';
  }

  if (updateBox) {
    if (updateAvailable) {
      updateBox.textContent = updateVersion ? `Install v${updateVersion}` : 'Install Update';
      updateBox.classList.add('show');
    } else {
      updateBox.classList.remove('show');
    }
  }

  if (settingsUpdateBtn) {
    if (updateAvailable) {
      settingsUpdateBtn.textContent = updateVersion ? `Install v${updateVersion}` : 'Install Update';
      settingsUpdateBtn.style.display = '';
    } else {
      settingsUpdateBtn.style.display = 'none';
    }
  }
}

async function loadVersion() {
  try {
    const version = await invoke('get_app_version');
    updateVersionDisplay({ version, isLatest: true });
  } catch (e) {
    console.error('Failed to load version:', e);
  }
}

async function checkForUpdates() {
  try {
    console.log('Checking for updates...');
    const currentVersion = await invoke('get_app_version');
    const result = await invoke('check_for_updates');
    console.log('Update check result:', JSON.stringify(result));

    updateVersionDisplay({
      version: currentVersion,
      isLatest: !result?.available,
      publishedAt: result?.published_at || '',
      updateAvailable: !!result?.available,
      updateVersion: result?.version || '',
    });
  } catch (e) {
    console.error('Failed to check for updates:', e);
    const updateBox = document.getElementById('updateBox');
    if (updateBox) updateBox.classList.remove('show');
  }
}

// Plugin Toggle Functions
let selectedPluginFolders = new Set();

function initAdminSelect(selectEl) {
  if (!selectEl || selectEl.dataset.adminSelectWired === 'true') return;
  selectEl.dataset.adminSelectWired = 'true';

  const wrap = document.createElement('div');
  wrap.className = 'admin-select';
  selectEl.parentNode.insertBefore(wrap, selectEl);
  wrap.appendChild(selectEl);

  const trigger = document.createElement('button');
  trigger.type = 'button';
  trigger.className = 'admin-select-trigger';
  trigger.setAttribute('aria-haspopup', 'listbox');
  trigger.setAttribute('aria-expanded', 'false');
  trigger.innerHTML = `
    <span class="admin-select-value"></span>
    <svg class="admin-select-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
      <path d="m6 9 6 6 6-6"/>
    </svg>`;
  wrap.appendChild(trigger);

  const menu = document.createElement('div');
  menu.className = 'admin-select-menu';
  menu.setAttribute('role', 'listbox');
  menu.hidden = true;
  wrap.appendChild(menu);

  const valueEl = trigger.querySelector('.admin-select-value');

  function closeMenu() {
    menu.hidden = true;
    trigger.setAttribute('aria-expanded', 'false');
  }

  function syncFromSelect() {
    const opt = selectEl.options[selectEl.selectedIndex];
    valueEl.textContent = opt ? opt.textContent : '';
    valueEl.classList.toggle('is-placeholder', !selectEl.value);
    menu.querySelectorAll('.admin-select-item').forEach(item => {
      item.classList.toggle('is-selected', item.dataset.value === selectEl.value);
    });
  }

  function rebuildMenu() {
    menu.innerHTML = '';
    [...selectEl.options].forEach(opt => {
      const item = document.createElement('button');
      item.type = 'button';
      item.className = 'admin-select-item';
      item.dataset.value = opt.value;
      item.setAttribute('role', 'option');
      item.textContent = opt.textContent;
      item.addEventListener('click', (e) => {
        e.stopPropagation();
        selectEl.value = opt.value;
        selectEl.dispatchEvent(new Event('change', { bubbles: true }));
        closeMenu();
        syncFromSelect();
      });
      menu.appendChild(item);
    });
    syncFromSelect();
  }

  trigger.addEventListener('click', (e) => {
    e.stopPropagation();
    const willOpen = menu.hidden;
    document.querySelectorAll('.admin-select-menu').forEach(m => { m.hidden = true; });
    document.querySelectorAll('.admin-select-trigger').forEach(t => t.setAttribute('aria-expanded', 'false'));
    if (willOpen) {
      menu.hidden = false;
      trigger.setAttribute('aria-expanded', 'true');
    }
  });

  if (!window._adminSelectCloseListener) {
    document.addEventListener('click', () => {
      document.querySelectorAll('.admin-select-menu').forEach(m => { m.hidden = true; });
      document.querySelectorAll('.admin-select-trigger').forEach(t => t.setAttribute('aria-expanded', 'false'));
    });
    window._adminSelectCloseListener = true;
  }

  selectEl.addEventListener('change', syncFromSelect);
  new MutationObserver(rebuildMenu).observe(selectEl, { childList: true });
  rebuildMenu();
}

async function loadPluginToggleServers() {
  try {
    const servers = await invoke('get_plugin_server_roots');
    const select = document.getElementById('pluginToggleServerSelect');
    select.innerHTML = '<option value="">Select a server</option>';
    
    servers.forEach(server => {
      const option = document.createElement('option');
      option.value = server;
      option.textContent = server;
      select.appendChild(option);
    });
  } catch (e) {
    console.error('Failed to load servers:', e);
  }
}

async function loadPluginFolders(serverRoot) {
  try {
    const folders = await invoke('list_plugin_folders', { serverRoot });
    const container = document.getElementById('pluginToggleFoldersList');
    container.innerHTML = '';
    
    if (folders.length === 0) {
      container.innerHTML = '<div class="empty-state">No plugin folders found</div>';
      return;
    }
    
    folders.forEach(folder => {
      const item = document.createElement('div');
      item.className = `plugin-folder-item ${folder.is_disabled ? 'disabled' : ''}`;
      
      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.value = folder.full_path;
      checkbox.dataset.baseName = folder.base_name;
      checkbox.addEventListener('change', updateToggleButtons);
      
      const label = document.createElement('label');
      label.className = 'folder-name';
      label.textContent = folder.name;
      label.style.cursor = 'pointer';
      label.style.margin = 0;
      label.style.flex = 1;
      
      label.addEventListener('click', (e) => {
        e.preventDefault();
        checkbox.checked = !checkbox.checked;
        checkbox.dispatchEvent(new Event('change'));
      });
      
      item.appendChild(checkbox);
      item.appendChild(label);
      container.appendChild(item);
    });
    
    selectedPluginFolders.clear();
    updateToggleButtons();
  } catch (e) {
    console.error('Failed to load plugin folders:', e);
    const container = document.getElementById('pluginToggleFoldersList');
    container.innerHTML = `<div class="empty-state" style="color: var(--error-color);">Error: ${e}</div>`;
  }
}

function updateToggleButtons() {
  const checkboxes = document.querySelectorAll('#pluginToggleFoldersList input[type="checkbox"]:checked');
  const hasSelection = checkboxes.length > 0;
  
  document.getElementById('toggleCurrentServerBtn').disabled = !hasSelection;
  document.getElementById('toggleAllServersBtn').disabled = !hasSelection;
}

async function togglePluginsForCurrentServer() {
  console.log('togglePluginsForCurrentServer called');
  const checkboxes = document.querySelectorAll('#pluginToggleFoldersList input[type="checkbox"]:checked');
  console.log('Selected checkboxes:', checkboxes.length);
  
  if (checkboxes.length === 0) {
    alert('Please select at least one folder to toggle');
    return;
  }
  
  try {
    const confirmed = await confirm(`Toggle ${checkboxes.length} folder(s) for the current server?`, {
      title: 'Toggle Plugins',
      kind: 'info'
    });
    
    if (!confirmed) {
      console.log('User cancelled toggle');
      return;
    }
    
    const serverRoot = document.getElementById('pluginToggleServerSelect').value;
    console.log('Server root:', serverRoot);
    const errors = [];
    let successCount = 0;
    
    for (const checkbox of checkboxes) {
      try {
        console.log('Toggling folder:', checkbox.value);
        const newPath = await invoke('toggle_plugin_folder', { folderPath: checkbox.value });
        console.log('Folder toggled successfully, new path:', newPath);
        successCount++;
      } catch (e) {
        console.error('Error toggling folder:', checkbox.value, e);
        errors.push(`${checkbox.dataset.baseName}: ${e}`);
      }
    }
    
    if (errors.length > 0) {
      alert('Some folders failed to toggle:\n' + errors.join('\n'));
    } else {
      console.log(`Successfully toggled ${successCount} folder(s)`);
    }
    
    // Reload folders
    await loadPluginFolders(serverRoot);
  } catch (e) {
    console.error('Error in togglePluginsForCurrentServer:', e);
    alert('Failed to toggle plugins: ' + e);
  }
}

async function togglePluginsForAllServers() {
  console.log('togglePluginsForAllServers called');
  const checkboxes = document.querySelectorAll('#pluginToggleFoldersList input[type="checkbox"]:checked');
  console.log('Selected checkboxes:', checkboxes.length);
  
  if (checkboxes.length === 0) {
    alert('Please select at least one folder to toggle');
    return;
  }
  
  // Get the current state of selected folders from the current server
  const serverRoot = document.getElementById('pluginToggleServerSelect').value;
  if (!serverRoot) {
    alert('Please select a server first');
    return;
  }
  
  // Get current folder states
  const folders = await invoke('list_plugin_folders', { serverRoot });
  const folderStates = new Map();
  folders.forEach(folder => {
    folderStates.set(folder.base_name, folder.is_disabled);
  });
  
  // Determine target states based on current server
  const foldersToToggle = [];
  for (const checkbox of checkboxes) {
    const baseName = checkbox.dataset.baseName;
    const currentStateDisabled = folderStates.get(baseName) || false;
    const targetStateDisabled = !currentStateDisabled; // Toggle to opposite state
    
    foldersToToggle.push({
      baseName,
      targetStateDisabled,
      currentState: currentStateDisabled ? 'disabled' : 'enabled',
      targetState: targetStateDisabled ? 'disabled' : 'enabled'
    });
  }
  
  const uniqueFolders = [];
  const seen = new Set();
  for (const folder of foldersToToggle) {
    if (!seen.has(folder.baseName)) {
      seen.add(folder.baseName);
      uniqueFolders.push(folder);
    }
  }
  
  console.log('Folders to toggle across all servers:', uniqueFolders);
  
  const folderList = uniqueFolders.map(f => 
    `${f.baseName} (${f.currentState} → ${f.targetState})`
  ).join('\n');
  
  try {
    const confirmed = await confirm(
      `Set ${uniqueFolders.length} folder(s) across all servers to match current server state?\n\n` +
      `Folders:\n${folderList}`,
      {
        title: 'Toggle Plugins for All Servers',
        kind: 'info'
      }
    );
    
    if (!confirmed) {
      console.log('User cancelled toggle');
      return;
    }
    
    const errors = [];
    const toggledPaths = [];
    
    for (const folder of uniqueFolders) {
      try {
        console.log(`Setting ${folder.baseName} to ${folder.targetState ? 'disabled' : 'enabled'} across all servers`);
        const paths = await invoke('toggle_plugin_for_all_servers', { 
          baseFolderName: folder.baseName,
          targetStateDisabled: folder.targetStateDisabled
        });
        console.log('Toggled paths:', paths);
        toggledPaths.push(...paths);
      } catch (e) {
        console.error('Error toggling folder:', folder.baseName, e);
        errors.push(`${folder.baseName}: ${e}`);
      }
    }
    
    if (errors.length > 0) {
      alert('Some folders failed to toggle:\n' + errors.join('\n'));
    } else {
      alert(`Successfully set ${toggledPaths.length} folder(s) across all servers`);
    }
    
    // Reload folders for current server
    if (serverRoot) {
      await loadPluginFolders(serverRoot);
    }
  } catch (e) {
    console.error('Error in togglePluginsForAllServers:', e);
    alert('Failed to toggle plugins: ' + e);
  }
}

// Make functions globally accessible for onclick handlers (must be after function declarations)
window.togglePluginsForCurrentServer = togglePluginsForCurrentServer;
window.togglePluginsForAllServers = togglePluginsForAllServers;

async function handleUpdateClick() {
  const updateBox = document.getElementById('updateBox');
  if (!updateBox || !updateBox.classList.contains('show')) {
    return;
  }
  
  const confirmed = await confirm('Install update now? The application will restart after installation.', {
    title: 'Update Available',
    kind: 'info'
  });
  
  if (!confirmed) {
    return;
  }
  
  try {
    updateBox.textContent = 'Installing update...';
    updateBox.style.pointerEvents = 'none';
    
    await invoke('install_update');
    // App will restart automatically after installation
  } catch (e) {
    alert('Failed to install update: ' + e);
    updateBox.textContent = 'Update Available - Click to Install';
    updateBox.style.pointerEvents = 'auto';
  }
}
