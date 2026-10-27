import {
  closeEditDrawer, saveJobChanges, deleteJobFromDrawer, pickDrawerRootDir, pickDrawerDestinationDir,
} from './jobs/form.js';
import { showMonthlyStatus, closeMonthlyStatus, runMonthlyBackup, runMonthlyFromStatus } from './jobs/monthly.js';
import { refreshJobs } from './jobs/table.js';
import { clearLogsView, openLogsFolder } from './logs.js';
import {
  showAddJobForm, closeAddJobModal, wizardNextStep, wizardPreviousStep,
  pickWizardRootDir, pickWizardDestinationDir,
} from './wizard/backup-wizard.js';
import { browsePluginSource, installSelectedPlugins, closePluginResultsModal } from './plugins/install.js';
import { togglePluginsForCurrentServer, togglePluginsForAllServers } from './plugins/toggle.js';
import { searchDataLookup, deleteSelectedDataLookupFiles } from './data-lookup.js';

const actions = {
  refreshJobs,
  runMonthlyBackup,
  runMonthlyFromStatus,
  showAddJobForm,
  showMonthlyStatus,
  closeMonthlyStatus,
  closeEditDrawer: () => closeEditDrawer(),
  saveJobChanges,
  deleteJobFromDrawer,
  pickDrawerRootDir,
  pickDrawerDestinationDir,
  clearLogsView,
  openLogsFolder,
  browsePluginSource,
  installSelectedPlugins,
  closePluginResultsModal,
  togglePluginsForCurrentServer,
  togglePluginsForAllServers,
  searchDataLookup,
  deleteSelectedDataLookupFiles,
  closeAddJobModal,
  pickWizardRootDir,
  pickWizardDestinationDir,
  wizardPreviousStep,
  wizardNextStep,
};

export function bindUiActions() {
  document.addEventListener('click', (event) => {
    const target = event.target.closest('[data-action]');
    if (!target || target.disabled) return;
    const action = actions[target.dataset.action];
    if (typeof action === 'function') action();
  });
}
