const HTML_ESCAPES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

export function escapeHtml(str) {
  return String(str ?? '').replace(/[&<>"']/g, (c) => HTML_ESCAPES[c]);
}

export const escapeHtmlAttr = escapeHtml;

export function setSelectValue(selectEl, value) {
  if (!selectEl) return;
  const normalized = String(value ?? '');
  const hasOption = [...selectEl.options].some((opt) => opt.value === normalized);
  selectEl.value = hasOption ? normalized : (selectEl.options[0]?.value ?? '');
  if (typeof selectEl._syncSelect === 'function') {
    selectEl._syncSelect();
  }
}

export function populateMapSelect(selectEl, maps, selectedValue = '') {
  if (!selectEl) return;
  const current = selectedValue || selectEl.value;
  selectEl.innerHTML = '<option value="">Select a map</option>';
  maps.forEach(map => {
    const opt = document.createElement('option');
    opt.value = map.id;
    opt.textContent = map.display_name;
    selectEl.appendChild(opt);
  });
  setSelectValue(selectEl, current);
}

export function fillSelect(selectEl, options, selectedValue) {
  if (!selectEl) return;
  selectEl.innerHTML = options
    .map(({ value, label }) => `<option value="${escapeHtml(value)}">${escapeHtml(label)}</option>`)
    .join('');
  setSelectValue(selectEl, selectedValue ?? selectEl.options[0]?.value);
}

export function debounce(fn, wait = 250) {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), wait);
  };
}

/** Segmented controls are `.segmented` containers of `.segment[data-value]` buttons. */
export function setSegmented(container, value) {
  if (!container) return;
  container.querySelectorAll('.segment').forEach(seg => {
    const on = seg.dataset.value === value;
    seg.classList.toggle('active', on);
    seg.setAttribute('aria-checked', String(on));
  });
}

export function getSegmented(container) {
  return container?.querySelector('.segment.active')?.dataset.value ?? null;
}

export function onSegmentedChange(container, handler) {
  container?.addEventListener('click', (e) => {
    const seg = e.target.closest('.segment');
    if (!seg || !container.contains(seg) || seg.classList.contains('active')) return;
    setSegmented(container, seg.dataset.value);
    handler(seg.dataset.value);
  });
}

export function setSwitch(el, on) {
  el?.setAttribute('aria-checked', String(!!on));
}

export function getSwitch(el) {
  return el?.getAttribute('aria-checked') === 'true';
}

/** Wire a role="switch" button so clicking it flips aria-checked. */
export function bindSwitch(el, onChange) {
  el?.addEventListener('click', () => {
    const next = !getSwitch(el);
    setSwitch(el, next);
    onChange?.(next);
  });
}

/** Set a `.check` span to on / off / mixed. */
export function setCheck(el, state) {
  if (!el) return;
  el.classList.toggle('on', state === true);
  el.classList.toggle('mixed', state === 'mixed');
}

export const CHECK_HTML = '<span class="check"><svg viewBox="0 0 24 24" fill="none" stroke="var(--on-cyan)" stroke-width="4"><path d="M5 12l5 5L20 7"/></svg></span>';

export function checkHtml(on) {
  return on ? CHECK_HTML.replace('class="check"', 'class="check on"') : CHECK_HTML;
}

export function setMessage(el, text, type = 'info') {
  if (!el) return;
  el.hidden = !text;
  el.textContent = text || '';
  el.classList.remove('is-error', 'is-success', 'is-warning');
  if (type === 'error') el.classList.add('is-error');
  if (type === 'success') el.classList.add('is-success');
  if (type === 'warning') el.classList.add('is-warning');
}

export function setButtonBusy(btn, busy, busyLabel) {
  if (!btn) return;
  if (busy) {
    btn.dataset.label = btn.innerHTML;
    btn.disabled = true;
    if (busyLabel) btn.textContent = busyLabel;
  } else {
    if (btn.dataset.label !== undefined) btn.innerHTML = btn.dataset.label;
    delete btn.dataset.label;
    btn.disabled = false;
  }
}

export function storageGet(key, fallback) {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : JSON.parse(raw);
  } catch {
    return fallback;
  }
}

export function storageSet(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage unavailable; the preference just won't persist.
  }
}
