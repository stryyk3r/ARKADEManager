// Shared mutable application state
export const state = {
  currentJobId: null,
  currentJobType: 'ark', // 'ark' | 'minecraft' | 'palworld' - used when editing
  jobsRefreshInterval: null,
  statusUpdateInterval: null,
  logsRefreshInterval: null,
  updateCheckInterval: null,
  allJobs: [],
  currentRunningJob: null,
  dataLookupResults: [],
  editableArkMaps: [],
  currentWizardStep: 1,
  backupType: null, // 'ark' | 'minecraft' | 'palworld' - set when user completes step 1
  pluginSourcePath: null,
  pluginSourcePlugins: [],
  pluginDestinations: [],
  pendingInstallation: null, // Store installation data for confirmation
  selectedPluginFolders: new Set(),
};

export const GITHUB_RELEASES_URL = 'https://github.com/stryyk3r/ARKADEManager/releases';
