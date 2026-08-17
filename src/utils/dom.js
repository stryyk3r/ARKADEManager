export function setSelectValue(selectEl, value) {
  if (!selectEl) return;
  const normalized = String(value ?? '');
  const hasOption = normalized && [...selectEl.options].some((opt) => opt.value === normalized);
  selectEl.value = hasOption ? normalized : '';
  if (typeof selectEl._syncAdminSelect === 'function') {
    selectEl._syncAdminSelect();
  } else {
    selectEl.dispatchEvent(new Event('change', { bubbles: true }));
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

export function escapeHtmlAttr(value) {
  return String(value ?? '')
    .replace(/&/g, '&amp;')
    .replace(/"/g, '&quot;')
    .replace(/</g, '&lt;');
}

export function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str || '';
  return div.innerHTML;
}
