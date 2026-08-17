import { state } from '../../state.js';

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

export function applyJobFilters() {
  document.querySelectorAll('#jobsTableBody tr[data-job-id]').forEach(row => {
    const jobId = row.dataset.jobId;
    const job = state.allJobs.find(j => j.id === jobId);
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
  if (jobsPanel) jobsPanel.style.display = state.allJobs.length > 0 ? '' : 'none';
}
