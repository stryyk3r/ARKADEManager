import { state, getJobGroupKey } from '../../state.js';
import { onSegmentedChange } from '../../utils/dom.js';
import { getJobRowState, updateToggleAllLabel } from './table.js';

function jobMatchesFilter(job) {
  const { game, status, text } = state.jobFilter;

  if (game !== 'all' && getJobGroupKey(job) !== game) return false;

  if (status !== 'all') {
    const kind = getJobRowState(job).kind;
    if (kind !== status) return false;
  }

  const search = text.toLowerCase().trim();
  if (search) {
    const haystack = [
      job.name,
      job.map,
      job.monthly_cluster,
      job.root_dir,
      job.destination_dir,
      job.job_type || 'ark',
    ].join(' ').toLowerCase();
    if (!haystack.includes(search)) return false;
  }
  return true;
}

export function applyJobFilters() {
  let visible = 0;
  document.querySelectorAll('#jobsTableBody .job-row[data-job-id]').forEach(row => {
    const job = state.allJobs.find(j => j.id === row.dataset.jobId);
    const show = job ? jobMatchesFilter(job) : false;
    row.classList.toggle('hidden-row', !show);
    if (show) visible++;
  });

  document.querySelectorAll('#jobsTableBody .job-group').forEach(group => {
    const anyVisible = group.querySelector('.job-row:not(.hidden-row)') !== null;
    group.classList.toggle('hidden-row', !anyVisible);
  });

  const noMatch = document.getElementById('jobsNoMatch');
  if (noMatch) noMatch.hidden = visible > 0 || state.allJobs.length === 0;
  updateToggleAllLabel();
}

export function setupJobFilters() {
  const search = document.getElementById('jobSearchInput');
  search?.addEventListener('input', () => {
    state.jobFilter.text = search.value;
    applyJobFilters();
  });
  document.getElementById('jobStatusFilter')?.addEventListener('change', (e) => {
    state.jobFilter.status = e.target.value;
    applyJobFilters();
  });
  onSegmentedChange(document.getElementById('jobGameFilter'), (value) => {
    state.jobFilter.game = value;
    applyJobFilters();
  });
}
