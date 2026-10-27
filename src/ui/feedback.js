import { escapeHtml } from '../utils/dom.js';
import { closeMenu } from './menu.js';

// ---------- Overlays (modals, drawer) ----------

const overlayStack = [];

const FOCUSABLE = 'button:not(:disabled), [href], input:not(:disabled):not([type="hidden"]), select:not(:disabled):not(.select-native), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

/**
 * Show an overlay element (`.overlay` or `.drawer-overlay`). Esc and a click on the scrim call
 * `onDismiss` (default: close it). Focus moves inside and returns to the opener on close.
 */
export function openOverlay(el, { onDismiss, initialFocus } = {}) {
  if (!el || overlayStack.some(entry => entry.el === el)) return;
  overlayStack.push({ el, onDismiss: onDismiss || (() => closeOverlay(el)), returnFocus: document.activeElement });
  el.classList.add('show');
  el.setAttribute('aria-hidden', 'false');
  requestAnimationFrame(() => {
    const target = typeof initialFocus === 'string' ? el.querySelector(initialFocus) : initialFocus;
    (target || el.querySelector(FOCUSABLE))?.focus({ preventScroll: true });
  });
}

export function closeOverlay(el) {
  const index = overlayStack.findIndex(entry => entry.el === el);
  if (index === -1) return;
  const [entry] = overlayStack.splice(index, 1);
  el.classList.remove('show');
  el.setAttribute('aria-hidden', 'true');
  closeMenu();
  if (entry.returnFocus && document.contains(entry.returnFocus)) {
    entry.returnFocus.focus({ preventScroll: true });
  }
}

export function isOverlayOpen(el) {
  return overlayStack.some(entry => entry.el === el);
}

document.addEventListener('keydown', (e) => {
  const top = overlayStack[overlayStack.length - 1];
  if (!top) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    top.onDismiss();
    return;
  }
  if (e.key === 'Tab') {
    const focusables = [...top.el.querySelectorAll(FOCUSABLE)].filter(n => n.offsetParent !== null);
    if (focusables.length === 0) return;
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    if (!top.el.contains(document.activeElement)) {
      e.preventDefault();
      first.focus();
    } else if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }
});

document.addEventListener('mousedown', (e) => {
  const top = overlayStack[overlayStack.length - 1];
  if (top && e.target === top.el) top.onDismiss();
});

// ---------- Toasts ----------

/**
 * Show a short, non-blocking message. kind: 'info' | 'success' | 'warning' | 'error'.
 * `detail` is a second, quieter line.
 */
export function toast(message, { kind = 'info', detail = '', duration } = {}) {
  const host = document.getElementById('toasts');
  if (!host) return;
  const el = document.createElement('div');
  el.className = `toast ${kind}`;
  el.innerHTML = `
    <span class="dot"></span>
    <div class="toast-body">${escapeHtml(message)}${detail ? `<span class="toast-detail">${escapeHtml(detail)}</span>` : ''}</div>
    <button class="toast-close" type="button" aria-label="Dismiss">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg>
    </button>`;
  host.appendChild(el);

  const dismiss = () => {
    if (!el.isConnected) return;
    el.classList.add('leaving');
    setTimeout(() => el.remove(), 160);
  };
  el.querySelector('.toast-close').addEventListener('click', dismiss);
  setTimeout(dismiss, duration ?? (kind === 'error' ? 9000 : 4500));

  while (host.children.length > 4) host.firstElementChild.remove();
}

export function toastError(prefix, error) {
  toast(prefix, { kind: 'error', detail: String(error ?? '') });
}

// ---------- Confirm dialog ----------

let pendingConfirm = null;

/**
 * In-app confirmation. `body` is trusted HTML (escape any user data before passing it).
 * Resolves true when confirmed.
 */
export function confirmDialog({ title, body = '', confirmLabel = 'Confirm', danger = false }) {
  const overlay = document.getElementById('confirmDialog');
  const okBtn = document.getElementById('confirmOkBtn');
  const cancelBtn = document.getElementById('confirmCancelBtn');
  if (pendingConfirm) pendingConfirm(false);

  document.getElementById('confirmTitle').textContent = title;
  document.getElementById('confirmBody').innerHTML = body;
  okBtn.textContent = confirmLabel;
  okBtn.className = `btn ${danger ? 'btn-danger' : 'btn-primary'}`;

  return new Promise((resolve) => {
    const finish = (result) => {
      if (pendingConfirm !== finish) return;
      pendingConfirm = null;
      okBtn.removeEventListener('click', onOk);
      cancelBtn.removeEventListener('click', onCancel);
      closeOverlay(overlay);
      resolve(result);
    };
    const onOk = () => finish(true);
    const onCancel = () => finish(false);
    pendingConfirm = finish;
    okBtn.addEventListener('click', onOk);
    cancelBtn.addEventListener('click', onCancel);
    openOverlay(overlay, { onDismiss: onCancel, initialFocus: danger ? cancelBtn : okBtn });
  });
}

/** Format a list for a confirm body: first `max` items, then "and N more". */
export function listHtml(items, max = 8) {
  const shown = items.slice(0, max).map(item => `<li>${escapeHtml(item)}</li>`).join('');
  const more = items.length > max ? `<li>…and ${items.length - max} more</li>` : '';
  return `<ul>${shown}${more}</ul>`;
}
