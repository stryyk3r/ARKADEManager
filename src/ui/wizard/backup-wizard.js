import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { state } from '../../state.js';
import { refreshJobs } from '../jobs/table.js';

const WIZARD_GAME_LABELS = {
  ark: {
    name: 'ARK: Survival Ascended',
    rootDescription: 'Select the root directory of your ARK: Survival Ascended server installation.'
  },
  minecraft: {
    name: 'Minecraft',
    rootDescription: 'Select the root directory of your Minecraft server (the folder containing world, config, and mods).'
  },
  palworld: {
    name: 'Palworld',
    rootDescription: 'Select the root directory of your Palworld dedicated server (the folder containing Pal).'
  }
};;

export function getTotalWizardSteps() {
  if (state.backupType === 'minecraft' || state.backupType === 'palworld') return 4;
  return 5;
}

export function getSelectedWizardBackupType() {
  const checked = document.querySelector('input[name="wizardBackupType"]:checked');
  return checked ? checked.value : null;
}

export function updateWizardLabels() {
  const step2 = document.getElementById('wizardStep2');
  if (!step2) return;

  const descEl = document.getElementById('wizardStep2Description')
    || step2.querySelector('.step-description');
  const titleEl = step2.querySelector('.step-title');
  const type = state.backupType || getSelectedWizardBackupType();
  const labels = type ? WIZARD_GAME_LABELS[type] : null;

  if (descEl) {
    descEl.textContent = labels
      ? labels.rootDescription
      : 'Select the root directory of your server installation.';
  }
  if (titleEl) {
    titleEl.textContent = labels
      ? `Step 2: ${labels.name} Server Root Directory`
      : 'Step 2: Server Root Directory';
  }
}

export function initWizardBackupTypeListeners() {
  document.querySelectorAll('input[name="wizardBackupType"]').forEach((radio) => {
    radio.addEventListener('change', () => {
      if (radio.checked) {
        state.backupType = radio.value;
        updateWizardLabels();
      }
    });
  });
}

export function showAddJobForm() {
  state.currentJobId = null;
  state.currentWizardStep = 1;
  state.backupType = null;
  resetWizard();
  document.getElementById('addJobModal').classList.add('show');
  updateWizardStep();
};

export function closeAddJobModal() {
  document.getElementById('addJobModal').classList.remove('show');
  resetWizard();
};

export function resetWizard() {
  state.currentWizardStep = 1;
  state.backupType = null;
  document.getElementById('wizardBackupTypeArk').checked = false;
  document.getElementById('wizardBackupTypeMinecraft').checked = false;
  document.getElementById('wizardBackupTypePalworld').checked = false;
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
  const pwName = document.getElementById('wizardPalworldJobName');
  const pwInterval = document.getElementById('wizardPalworldIntervalValue');
  const pwUnit = document.getElementById('wizardPalworldIntervalUnit');
  const pwRetention = document.getElementById('wizardPalworldRetentionDays');
  const pwEnabled = document.getElementById('wizardPalworldEnabled');
  const pwApiHost = document.getElementById('wizardPalworldApiHost');
  const pwCluster = document.getElementById('wizardMonthlyClusterPalworld');
  if (pwName) pwName.value = '';
  if (pwInterval) pwInterval.value = '1';
  if (pwUnit) pwUnit.value = 'minutes';
  if (pwRetention) pwRetention.value = '7';
  if (pwEnabled) pwEnabled.checked = false;
  if (pwApiHost) pwApiHost.value = '';
  if (pwCluster) pwCluster.value = 'Palworld';
  clearWizardErrors();
  updateWizardLabels();
}

export function clearWizardErrors() {
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
  const pwErr = document.getElementById('wizardPalworldJobNameError');
  if (pwErr) pwErr.textContent = '';
  const pwClusterErr = document.getElementById('wizardMonthlyClusterPalworldError');
  if (pwClusterErr) pwClusterErr.textContent = '';
}

export function updateWizardStep() {
  const totalSteps = getTotalWizardSteps();
  const stepIds = ['wizardStep1', 'wizardStep2', 'wizardStep3', 'wizardStep4', 'wizardStep5', 'wizardStepMinecraftConfig', 'wizardStepPalworldConfig'];
  stepIds.forEach(id => {
    const el = document.getElementById(id);
    if (el) el.classList.remove('active');
  });

  // Show current step panel
  if (state.currentWizardStep === 1) {
    document.getElementById('wizardStep1').classList.add('active');
  } else if (state.backupType === 'minecraft' && state.currentWizardStep === 4) {
    document.getElementById('wizardStepMinecraftConfig').classList.add('active');
  } else if (state.backupType === 'palworld' && state.currentWizardStep === 4) {
    document.getElementById('wizardStepPalworldConfig').classList.add('active');
  } else {
    document.getElementById(`wizardStep${state.currentWizardStep}`).classList.add('active');
  }

  // Update step indicators (show 2 dots for Minecraft, 5 for Ark / step 1)
  for (let i = 1; i <= 5; i++) {
    const dot = document.getElementById(`step${i}Dot`);
    const connector = document.getElementById(`connector${i}`);
    const visible = i <= totalSteps;
    if (dot) {
      dot.style.display = visible ? '' : 'none';
      dot.classList.remove('active', 'completed');
      if (i < state.currentWizardStep) dot.classList.add('completed');
      else if (i === state.currentWizardStep) dot.classList.add('active');
    }
    if (connector) {
      connector.style.display = visible && i < totalSteps ? '' : 'none';
      connector.classList.toggle('completed', i < state.currentWizardStep);
    }
  }

  // Update buttons
  document.getElementById('wizardPrevBtn').style.display = state.currentWizardStep > 1 ? 'block' : 'none';
  document.getElementById('wizardNextBtn').style.display = state.currentWizardStep < totalSteps ? 'block' : 'none';
  document.getElementById('wizardFinishBtn').style.display = state.currentWizardStep === totalSteps ? 'block' : 'none';

  updateWizardLabels();
}

export function wizardNextStep() {
  if (validateCurrentStep()) {
    const totalSteps = getTotalWizardSteps();
    if (state.currentWizardStep < totalSteps) {
      state.currentWizardStep++;
      updateWizardStep();
    }
  }
};

export function wizardPreviousStep() {
  if (state.currentWizardStep > 1) {
    state.currentWizardStep--;
    if (state.currentWizardStep === 1) state.backupType = null;
    updateWizardStep();
  }
};

export function validateCurrentStep() {
  clearWizardErrors();
  let isValid = true;

  switch (state.currentWizardStep) {
    case 1: {
      const arkChecked = document.getElementById('wizardBackupTypeArk').checked;
      const minecraftChecked = document.getElementById('wizardBackupTypeMinecraft').checked;
      const palworldChecked = document.getElementById('wizardBackupTypePalworld').checked;
      if (!arkChecked && !minecraftChecked && !palworldChecked) {
        document.getElementById('wizardBackupTypeError').textContent = 'Please choose ARK, Minecraft, or Palworld';
        isValid = false;
      } else {
        state.backupType = getSelectedWizardBackupType();
        updateWizardLabels();
      }
      break;
    }
    case 2: {
      const rootDir = document.getElementById('wizardRootDir').value.trim();
      if (!rootDir) {
        document.getElementById('wizardRootDirError').textContent = 'Server root directory is required';
        isValid = false;
      }
      break;
    }
    case 3: {
      const destDir = document.getElementById('wizardDestinationDir').value.trim();
      if (!destDir) {
        document.getElementById('wizardDestinationDirError').textContent = 'Destination directory is required';
        isValid = false;
      }
      break;
    }
    case 4:
      if (state.backupType === 'minecraft') {
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
      } else if (state.backupType === 'palworld') {
        const pwJobName = document.getElementById('wizardPalworldJobName').value.trim();
        if (!pwJobName) {
          document.getElementById('wizardPalworldJobNameError').textContent = 'Job name is required';
          isValid = false;
        }
        const pwCluster = document.getElementById('wizardMonthlyClusterPalworld')?.value;
        if (!pwCluster) {
          document.getElementById('wizardMonthlyClusterPalworldError').textContent = 'Monthly cluster is required';
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

export async function pickWizardRootDir() {
  try {
    const gameName = state.backupType && WIZARD_GAME_LABELS[state.backupType]
      ? WIZARD_GAME_LABELS[state.backupType].name
      : 'Server';
    const selected = await open({
      directory: true,
      multiple: false,
      title: `Select ${gameName} Root Directory`
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

export async function pickWizardDestinationDir() {
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

export async function wizardFinish() {
  if (!validateCurrentStep()) {
    return;
  }

  let job;
  if (state.backupType === 'minecraft') {
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
  } else if (state.backupType === 'palworld') {
    job = {
      job_type: 'palworld',
      monthly_cluster: document.getElementById('wizardMonthlyClusterPalworld').value,
      name: document.getElementById('wizardPalworldJobName').value.trim(),
      root_dir: document.getElementById('wizardRootDir').value.trim(),
      destination_dir: document.getElementById('wizardDestinationDir').value.trim(),
      map: '',
      include_saves: false,
      include_map: false,
      include_server_files: false,
      include_plugin_configs: false,
      interval_value: parseInt(document.getElementById('wizardPalworldIntervalValue').value) || 1,
      interval_unit: document.getElementById('wizardPalworldIntervalUnit').value,
      retention_days: parseInt(document.getElementById('wizardPalworldRetentionDays').value) || 7,
      enabled: document.getElementById('wizardPalworldEnabled').checked,
      rcon_host: document.getElementById('wizardPalworldApiHost').value.trim() || null,
      rcon_port: null,
      rcon_password: null
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
