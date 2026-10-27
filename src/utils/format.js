const MINUTE = 60 * 1000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function formatFileSize(bytes) {
  if (!bytes) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const value = bytes / Math.pow(1024, i);
  const decimals = i <= 1 ? 0 : i === 2 ? 1 : 2;
  return `${value.toFixed(decimals)} ${units[i]}`;
}

export function formatReleaseDate(isoDate) {
  const date = parseDate(isoDate);
  if (!date) return '';
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
}

export function parseDate(value) {
  if (!value) return null;
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

export function plural(count, singular, pluralForm = `${singular}s`) {
  return `${count.toLocaleString()} ${count === 1 ? singular : pluralForm}`;
}

function isSameDay(a, b) {
  return a.getFullYear() === b.getFullYear()
    && a.getMonth() === b.getMonth()
    && a.getDate() === b.getDate();
}

/** "8:31 PM" today, otherwise "Oct 26". */
export function formatClockOrDate(value, now = new Date()) {
  const date = parseDate(value);
  if (!date) return '—';
  if (isSameDay(date, now)) {
    return date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  }
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

/** "Oct 27, 7:41 PM" */
export function formatDateTime(value) {
  const date = parseDate(value);
  if (!date) return '—';
  return date.toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
}

export function formatTime(value) {
  const date = parseDate(value);
  if (!date) return '—';
  return date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
}

export function formatFullDate(value) {
  const date = parseDate(value);
  return date ? date.toLocaleString() : '';
}

function formatSpan(ms) {
  if (ms < MINUTE) return null;
  if (ms < HOUR) return `${Math.round(ms / MINUTE)} min`;
  if (ms < DAY) return `${Math.round(ms / HOUR)} h`;
  return `${Math.round(ms / DAY)} d`;
}

/** "in 12 min", "18 min ago", "just now", "due". */
export function formatRelative(value, now = Date.now()) {
  const date = parseDate(value);
  if (!date) return '';
  const diff = date.getTime() - now;
  const span = formatSpan(Math.abs(diff));
  if (diff >= 0) return span ? `in ${span}` : 'in <1 min';
  return span ? `${span} ago` : 'just now';
}

/** Like formatRelative, but a time in the past reads "due" (for next-run times). */
export function formatUntil(value, now = Date.now()) {
  const date = parseDate(value);
  if (!date) return '';
  return date.getTime() - now <= 0 ? 'due' : formatRelative(date, now);
}

export function formatInterval(value, unit) {
  const n = Number(value) || 0;
  switch (unit) {
    case 'minutes': return `${n} min`;
    case 'hours': return `${n} h`;
    case 'days': return n === 1 ? '1 day' : `${n} days`;
    default: return `${n} ${unit || ''}`.trim();
  }
}

export function formatIntervalLong(value, unit) {
  const n = Number(value) || 0;
  const word = { minutes: 'minute', hours: 'hour', days: 'day' }[unit] || unit;
  return `Every ${n} ${n === 1 ? word : `${word}s`}`;
}

/** Backup file timestamp, as written by the backend (UTC, YYYYMMDD_HHMMSS). */
export function formatBackupStamp(date = new Date()) {
  const p = (n) => String(n).padStart(2, '0');
  return `${date.getUTCFullYear()}${p(date.getUTCMonth() + 1)}${p(date.getUTCDate())}_`
    + `${p(date.getUTCHours())}${p(date.getUTCMinutes())}${p(date.getUTCSeconds())}`;
}

/** Shorten a Windows path to its last `keep` segments: "…\lna-center\ShooterGame\…". */
export function shortenPath(path, keep = 4) {
  const parts = String(path || '').split(/[\\/]/).filter(Boolean);
  if (parts.length <= keep + 1) return path || '';
  return `…\\${parts.slice(-keep).join('\\')}`;
}

export function basename(path) {
  const parts = String(path || '').split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || '';
}

export function joinPath(...parts) {
  return parts
    .filter(Boolean)
    .map((p, i) => (i === 0 ? String(p).replace(/[\\/]+$/, '') : String(p).replace(/^[\\/]+|[\\/]+$/g, '')))
    .join('\\');
}

export function normalizePathKey(path) {
  return String(path || '').trim().replace(/[\\/]+$/, '').replace(/\//g, '\\').toLowerCase();
}
