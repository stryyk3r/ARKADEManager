import { invoke } from '@tauri-apps/api/core';
import { open, confirm } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';

let currentJobId = null;
let jobsRefreshInterval = null;
let statusUpdateInterval = null;
let logsRefreshInterval = null;

// Initialize app
document.addEventListener('DOMContentLoaded', async () => {
  // Load theme
  try {
    const config = await invoke('get_config');
    if (config.theme) {
      document.documentElement.setAttribute('data-theme', config.theme);
    }
  } catch (e) {
    console.error('Failed to load config:', e);
  }
  
  // Setup tab switching
  document.querySelectorAll('.tab').forEach(tab => {
    tab.addEventListener('click', () => {
      const tabName = tab.dataset.tab;
      switchTab(tabName);
    });
  });
  
  // Theme toggle
  document.getElementById('themeToggle').addEventListener('click', async () => {
    const currentTheme = document.documentElement.getAttribute('data-theme') || 'light';
    const newTheme = currentTheme === 'light' ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', newTheme);
    try {
      await invoke('set_theme', { theme: newTheme });
    } catch (e) {
      console.error('Failed to save theme:', e);
    }
  });
  
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
    refreshJobs();
  });
  
  // Initial status and logs
  await updateStatus();
  await refreshLogs();
  
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
  document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
  document.querySelectorAll('.tab-pane').forEach(p => p.classList.remove('active'));
  
  document.querySelector(`[data-tab="${tabName}"]`).classList.add('active');
  document.getElementById(tabName).classList.add('active');
  
  // Load plugin destinations when new plugins tab is opened
  if (tabName === 'new-plugins' && typeof window.refreshPluginDestinations === 'function') {
    window.refreshPluginDestinations();
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
const totalWizardSteps = 4;

window.showAddJobForm = () => {
  currentJobId = null;
  currentWizardStep = 1;
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
  clearWizardErrors();
}

function clearWizardErrors() {
  document.getElementById('wizardRootDirError').textContent = '';
  document.getElementById('wizardDestinationDirError').textContent = '';
  document.getElementById('wizardMapError').textContent = '';
  document.getElementById('wizardJobNameError').textContent = '';
}

function updateWizardStep() {
  // Hide all steps
  for (let i = 1; i <= totalWizardSteps; i++) {
    document.getElementById(`wizardStep${i}`).classList.remove('active');
  }
  // Show current step
  document.getElementById(`wizardStep${currentWizardStep}`).classList.add('active');
  
  // Update step indicators
  for (let i = 1; i <= totalWizardSteps; i++) {
    const dot = document.getElementById(`step${i}Dot`);
    const connector = document.getElementById(`connector${i}`);
    if (i < currentWizardStep) {
      dot.classList.remove('active');
      dot.classList.add('completed');
      if (connector) connector.classList.add('completed');
    } else if (i === currentWizardStep) {
      dot.classList.add('active');
      dot.classList.remove('completed');
      if (connector) connector.classList.remove('completed');
    } else {
      dot.classList.remove('active', 'completed');
      if (connector) connector.classList.remove('completed');
    }
  }
  
  // Update buttons
  document.getElementById('wizardPrevBtn').style.display = currentWizardStep > 1 ? 'block' : 'none';
  document.getElementById('wizardNextBtn').style.display = currentWizardStep < totalWizardSteps ? 'block' : 'none';
  document.getElementById('wizardFinishBtn').style.display = currentWizardStep === totalWizardSteps ? 'block' : 'none';
}

window.wizardNextStep = () => {
  if (validateCurrentStep()) {
    if (currentWizardStep < totalWizardSteps) {
      currentWizardStep++;
      updateWizardStep();
    }
  }
};

window.wizardPreviousStep = () => {
  if (currentWizardStep > 1) {
    currentWizardStep--;
    updateWizardStep();
  }
};

function validateCurrentStep() {
  clearWizardErrors();
  let isValid = true;
  
  switch (currentWizardStep) {
    case 1:
      const rootDir = document.getElementById('wizardRootDir').value.trim();
      if (!rootDir) {
        document.getElementById('wizardRootDirError').textContent = 'Server root directory is required';
        isValid = false;
      }
      break;
    case 2:
      const destDir = document.getElementById('wizardDestinationDir').value.trim();
      if (!destDir) {
        document.getElementById('wizardDestinationDirError').textContent = 'Destination directory is required';
        isValid = false;
      }
      break;
    case 3:
      const map = document.getElementById('wizardMapSelect').value;
      if (!map) {
        document.getElementById('wizardMapError').textContent = 'Map selection is required';
        isValid = false;
      }
      break;
    case 4:
      const jobName = document.getElementById('wizardJobName').value.trim();
      if (!jobName) {
        document.getElementById('wizardJobNameError').textContent = 'Job name is required';
        isValid = false;
      }
      break;
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
  
  try {
    const job = {
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

window.clearForm = () => {
  document.getElementById('rootDir').value = '';
  document.getElementById('destinationDir').value = '';
  document.getElementById('mapSelect').value = '';
  document.getElementById('jobName').value = '';
  document.getElementById('includeSaves').checked = false;
  document.getElementById('includeMap').checked = false;
  document.getElementById('includeServerFiles').checked = false;
  document.getElementById('includePluginConfigs').checked = false;
  document.getElementById('intervalValue').value = '1';
  document.getElementById('intervalUnit').value = 'minutes';
  document.getElementById('retentionDays').value = '7';
  document.getElementById('enabled').checked = false;
  clearAllErrors();
};

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
    const preview = await invoke('preview_monthly_archive');
    if (preview.files && preview.files.length > 0) {
      alert(`Monthly Archive Preview:\n\n${preview.files.length} files would be archived:\n${preview.files.slice(0, 5).join('\n')}${preview.files.length > 5 ? '\n...' : ''}`);
    } else {
      alert('No files to archive this month');
    }
  } catch (e) {
    alert('Failed to get monthly status: ' + e);
  }
};

window.runMonthlyBackup = async () => {
  if (!confirm('Run monthly archive now? This will archive the oldest 2 backups from the current month.')) {
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
  const map = document.getElementById('mapSelect').value;
  const name = document.getElementById('jobName').value.trim();
  
  if (!rootDir) {
    showError('rootDirError', 'Server root directory is required');
    return null;
  }
  if (!destinationDir) {
    showError('destinationDirError', 'Destination directory is required');
    return null;
  }
  if (!map) {
    showError('mapError', 'Map selection is required');
    return null;
  }
  if (!name) {
    showError('jobNameError', 'Job name is required');
    return null;
  }
  
  clearAllErrors();
  
  return {
    name,
    root_dir: rootDir,
    destination_dir: destinationDir,
    map,
    include_saves: document.getElementById('includeSaves').checked,
    include_map: document.getElementById('includeMap').checked,
    include_server_files: document.getElementById('includeServerFiles').checked,
    include_plugin_configs: document.getElementById('includePluginConfigs').checked,
    interval_value: parseInt(document.getElementById('intervalValue').value) || 1,
    interval_unit: document.getElementById('intervalUnit').value,
    retention_days: parseInt(document.getElementById('retentionDays').value) || 7,
    enabled: document.getElementById('enabled').checked
  };
}

function renderJobsTable(jobs) {
  const tbody = document.getElementById('jobsTableBody');
  tbody.innerHTML = '';
  
  if (jobs.length === 0) {
    const row = tbody.insertRow();
    const cell = row.insertCell();
    cell.colSpan = 6;
    cell.textContent = 'No jobs configured';
    cell.style.textAlign = 'center';
    cell.style.color = 'var(--text-secondary)';
    return;
  }
  
  jobs.forEach(job => {
    const row = tbody.insertRow();
    row.dataset.jobId = job.id;
    
    row.insertCell().textContent = job.name;
    row.insertCell().textContent = `${job.interval_value} ${job.interval_unit}`;
    row.insertCell().textContent = job.next_run_at ? new Date(job.next_run_at).toLocaleString() : 'N/A';
    row.insertCell().textContent = job.last_run_at ? new Date(job.last_run_at).toLocaleString() : 'Never';
    row.insertCell().textContent = job.last_file_size ? formatFileSize(job.last_file_size) : 'N/A';
    
    // Add ellipsis menu
    const menuCell = row.insertCell();
    menuCell.style.textAlign = 'center';
    menuCell.style.padding = '4px';
    
    const menuContainer = document.createElement('div');
    menuContainer.className = 'job-menu';
    
    const menuButton = document.createElement('button');
    menuButton.className = 'job-menu-button';
    menuButton.textContent = '⋯';
    menuButton.onclick = (e) => {
      e.stopPropagation();
      // Close other menus
      document.querySelectorAll('.job-menu-dropdown').forEach(d => d.classList.remove('show'));
      const dropdown = menuContainer.querySelector('.job-menu-dropdown');
      dropdown.classList.toggle('show');
    };
    
    const dropdown = document.createElement('div');
    dropdown.className = 'job-menu-dropdown';
    
    const runItem = document.createElement('button');
    runItem.className = 'job-menu-item';
    runItem.textContent = 'Run Now';
    runItem.onclick = (e) => {
      e.stopPropagation();
      dropdown.classList.remove('show');
      runJobNow(job.id);
    };
    
    const editItem = document.createElement('button');
    editItem.className = 'job-menu-item';
    editItem.textContent = 'Edit';
    editItem.onclick = (e) => {
      e.stopPropagation();
      dropdown.classList.remove('show');
      updateJob(job.id);
    };
    
    const deleteItem = document.createElement('button');
    deleteItem.className = 'job-menu-item danger';
    deleteItem.textContent = 'Delete';
    deleteItem.onclick = (e) => {
      e.stopPropagation();
      dropdown.classList.remove('show');
      deleteJob(job.id);
    };
    
    dropdown.appendChild(runItem);
    dropdown.appendChild(editItem);
    dropdown.appendChild(deleteItem);
    
    menuContainer.appendChild(menuButton);
    menuContainer.appendChild(dropdown);
    menuCell.appendChild(menuContainer);
  });
  
  // Close menus when clicking outside
  document.addEventListener('click', (e) => {
    if (!e.target.closest('.job-menu')) {
      document.querySelectorAll('.job-menu-dropdown').forEach(d => d.classList.remove('show'));
    }
  });
}

function loadJobIntoForm(job) {
  currentJobId = job.id;
  document.getElementById('rootDir').value = job.root_dir || '';
  document.getElementById('destinationDir').value = job.destination_dir || '';
  document.getElementById('mapSelect').value = job.map || '';
  document.getElementById('jobName').value = job.name || '';
  document.getElementById('includeSaves').checked = job.include_saves || false;
  document.getElementById('includeMap').checked = job.include_map || false;
  document.getElementById('includeServerFiles').checked = job.include_server_files || false;
  document.getElementById('includePluginConfigs').checked = job.include_plugin_configs || false;
  document.getElementById('intervalValue').value = job.interval_value || 1;
  document.getElementById('intervalUnit').value = job.interval_unit || 'minutes';
  document.getElementById('retentionDays').value = job.retention_days || 7;
  document.getElementById('enabled').checked = job.enabled || false;
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

function updateStatusFromEvent(status) {
  const indicator = document.getElementById('runningIndicator');
  if (status.running) {
    indicator.classList.remove('stopped');
    document.getElementById('schedulerStatus').textContent = 'Running';
  } else {
    indicator.classList.add('stopped');
    document.getElementById('schedulerStatus').textContent = 'Stopped';
  }
  
  document.getElementById('queueSize').textContent = status.queue_size || 0;
  document.getElementById('currentJob').textContent = status.current_job || 'None';
  document.getElementById('lastTick').textContent = status.last_tick ? new Date(status.last_tick).toLocaleString() : 'Never';
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
  
  if (pluginSourcePlugins.length === 0) {
    container.innerHTML = '<div class="empty-state">No plugin folders found in source directory</div>';
    return;
  }
  
  container.innerHTML = pluginSourcePlugins.map((plugin, index) => `
    <div class="plugin-item">
      <input type="checkbox" id="source-plugin-${index}" data-path="${plugin.path}">
      <label for="source-plugin-${index}" class="plugin-item-label">${plugin.name}</label>
    </div>
  `).join('');
  
  // Attach event listeners to checkboxes
  container.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
    checkbox.addEventListener('change', updateInstallButtonState);
  });
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
  }
};

function renderDestinations() {
  const container = document.getElementById('pluginDestinationList');
  
  if (pluginDestinations.length === 0) {
    container.innerHTML = '<div class="empty-state">No ARK servers found in C:\\arkservers\\asaservers</div>';
    return;
  }
  
  container.innerHTML = pluginDestinations.map((server, index) => `
    <div class="plugin-item">
      <input type="checkbox" id="dest-server-${index}" data-path="${server.plugin_path}">
      <label for="dest-server-${index}" class="plugin-item-label">${server.name}</label>
    </div>
  `).join('');
  
  // Attach event listeners to checkboxes
  container.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
    checkbox.addEventListener('change', updateInstallButtonState);
  });
}

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
  
  // Close confirmation modal first
  closePluginConfirmModal();
  
  // Small delay to ensure modal is closed before showing results
  await new Promise(resolve => setTimeout(resolve, 100));
  
  const { sourcePaths, destPaths, selectedPlugins, selectedServers, sourceCheckboxes, destCheckboxes } = pendingInstallation;
  pendingInstallation = null;
  
  const installBtn = document.getElementById('installPluginsBtn');
  installBtn.disabled = true;
  installBtn.textContent = 'Installing...';
  
  try {
    console.log('Starting plugin installation...');
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
    installBtn.disabled = false;
    installBtn.textContent = 'Install Selected Plugins';
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
  html += '<strong>⚠ Note:</strong> Existing plugin files will be overwritten.';
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
  modal.classList.remove('show');
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

