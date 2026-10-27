import { state, DEFAULT_ASA_SERVER_ROOT } from '../../state.js';
import { joinPath, normalizePathKey } from '../../utils/format.js';

export function asaServerRoot() {
  return state.config?.asa_server_root || DEFAULT_ASA_SERVER_ROOT;
}

/** Monthly cluster of the ARK job that backs up this server folder, if any. */
export function clusterForServer(serverPath) {
  const key = normalizePathKey(serverPath);
  const job = state.allJobs.find(j => (j.job_type || 'ark') === 'ark' && normalizePathKey(j.root_dir) === key);
  return job?.monthly_cluster || '';
}

export function pluginsDirFor(serverRoot) {
  return joinPath(serverRoot, 'ShooterGame', 'Binaries', 'Win64', 'ArkApi', 'Plugins');
}
