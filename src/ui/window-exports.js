import {
  pickRootDir, pickDestinationDir, cancelJobForm, addJob,
  showMonthlyStatus, runMonthlyBackup
} from './jobs/form.js';
import { refreshJobs } from './jobs/table.js';
import { refreshLogs, clearLogsView, openLogsFolder } from './logs.js';
import {
  showAddJobForm, closeAddJobModal, wizardNextStep, wizardPreviousStep,
  pickWizardRootDir, pickWizardDestinationDir, wizardFinish
} from './wizard/backup-wizard.js';
import {
  browsePluginSource, refreshPluginDestinations, toggleAllSourcePlugins, toggleAllDestinations,
  installSelectedPlugins, proceedWithPluginInstallation,
  closePluginConfirmModal, closePluginResultsModal
} from './plugins/install.js';
import { togglePluginsForCurrentServer, togglePluginsForAllServers } from './plugins/toggle.js';
import { searchDataLookup, toggleDataLookupSelectAll, deleteSelectedDataLookupFiles } from './data-lookup.js';

const actions = {
  refreshJobs,
  runMonthlyBackup,
  showAddJobForm,
  showMonthlyStatus,
  pickRootDir,
  pickDestinationDir,
  addJob,
  cancelJobForm,
  refreshLogs,
  clearLogsView,
  openLogsFolder,
  toggleAllSourcePlugins,
  browsePluginSource,
  toggleAllDestinations,
  refreshPluginDestinations,
  installSelectedPlugins,
  togglePluginsForCurrentServer,
  togglePluginsForAllServers,
  searchDataLookup,
  toggleDataLookupSelectAll,
  deleteSelectedDataLookupFiles,
  closeAddJobModal,
  pickWizardRootDir,
  pickWizardDestinationDir,
  wizardPreviousStep,
  wizardNextStep,
  wizardFinish,
  closePluginConfirmModal,
  proceedWithPluginInstallation,
  closePluginResultsModal,
};

export function bindUiActions() {
  document.addEventListener('click', (event) => {
    const target = event.target.closest('[data-action]');
    if (!target) return;
    const action = actions[target.dataset.action];
    if (typeof action === 'function') {
      action();
    }
  });

  document.addEventListener('change', (event) => {
    const target = event.target.closest('[data-change]');
    if (!target) return;
    const action = actions[target.dataset.change];
    if (typeof action === 'function') {
      action();
    }
  });
}
