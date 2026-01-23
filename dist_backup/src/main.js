import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
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
});

function switchTab(tabName) {
  document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
  document.querySelectorAll('.tab-pane').forEach(p => p.classList.remove('active'));
  
  document.querySelector(`[data-tab="${tabName}"]`).classList.add('active');
  document.getElementById(tabName).classList.add('active');
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

window.showAddJobForm = () => {
  currentJobId = null;
  clearForm();
  const formContainer = document.getElementById('jobFormContainer');
  formContainer.style.display = 'block';
  formContainer.style.visibility = 'visible';
  formContainer.style.height = 'auto';
  formContainer.style.overflow = 'visible';
  formContainer.classList.add('show');
  document.getElementById('jobFormTitle').textContent = 'Add New Job';
  document.getElementById('submitJobBtn').textContent = 'Add Job';
  document.getElementById('submitJobBtn').onclick = addJob;
  // Scroll to form
  setTimeout(() => {
    formContainer.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }, 100);
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
  
  if (!confirm('Are you sure you want to delete this job?')) {
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

