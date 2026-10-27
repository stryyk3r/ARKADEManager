// Shared mutable application state
export const state = {
  jobsRefreshInterval: null,
  statusUpdateInterval: null,
  logsRefreshInterval: null,
  updateCheckInterval: null,

  config: null,
  arkMaps: [],

  // Backups
  allJobs: [],
  currentRunningJob: null, // name of the job the scheduler is running
  status: null, // last get_status / status_update payload
  backupProgress: null, // { jobName, percent } from backup_progress
  collapsedGroups: new Set(),
  jobFilter: { game: 'all', status: 'all', text: '' },
  jobMenuJobId: null,
  editJobId: null, // drawer open when not null

  // Add job wizard
  wizard: null,

  // Logs
  logFilter: { level: 'all', source: 'all', text: '', follow: true },
  logFiles: [],
  logFile: null, // selected file name; null = current
  logRaw: null,
  logEntries: [],
  logClearedAt: null,

  // Plugins
  pluginsTab: 'install',
  pluginSourcePath: null,
  pluginSourcePlugins: [],
  pluginDestinations: [],
  pluginInstall: { sourceSel: new Set(), destSel: new Set() },
  pluginServers: [], // [{ root, name, cluster, folders, error }]
  pluginToggle: { server: null, filter: 'all', search: '', pending: new Map() },

  // Data lookup
  dataLookupType: 'profile',
  dataLookupResults: [],
  lookupSelection: new Set(),

  // Settings
  editableArkMaps: [],
};

export const GITHUB_RELEASES_URL = 'https://github.com/stryyk3r/ARKADEManager/releases';

export const DEFAULT_ASA_SERVER_ROOT = 'C:\\arkservers\\asaservers';

export const MONTHLY_CLUSTERS = ['ASA Legacy', 'ASE Legacy', 'ASA Omega', 'Minecraft', 'Palworld'];

export const INTERVAL_UNITS = [
  { value: 'minutes', label: 'minutes' },
  { value: 'hours', label: 'hours' },
  { value: 'days', label: 'days' },
];

/** Display groups on the Backups page, in order. */
export const JOB_GROUP_ORDER = ['asa', 'ase', 'minecraft', 'palworld'];

export const GAMES = {
  asa: { label: 'ARK: Survival Ascended', short: 'ASA', letter: 'A', jobType: 'ark', cluster: 'ASA Legacy' },
  ase: { label: 'ARK: Survival Evolved', short: 'ASE', letter: 'A', jobType: 'ark', cluster: 'ASE Legacy' },
  minecraft: { label: 'Minecraft', short: 'Minecraft', letter: 'M', jobType: 'minecraft', cluster: 'Minecraft' },
  palworld: { label: 'Palworld', short: 'Palworld', letter: 'P', jobType: 'palworld', cluster: 'Palworld' },
};

export function getJobGroupKey(job) {
  if (job.job_type === 'minecraft') return 'minecraft';
  if (job.job_type === 'palworld') return 'palworld';
  const cluster = job.monthly_cluster || '';
  if (cluster === 'ASE Legacy') return 'ase';
  if (cluster === 'Palworld') return 'palworld';
  return 'asa';
}

export function gameChipHtml(groupKey, small = false) {
  const game = GAMES[groupKey] || GAMES.asa;
  return `<span class="game-chip ${small ? 'game-chip-sm ' : ''}game-${groupKey}">${game.letter}</span>`;
}
