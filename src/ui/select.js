import { openMenu, closeMenu, addMenuItem } from './menu.js';

const CHEVRON = '<svg class="select-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>';

/**
 * Replace a native <select>'s look with a styled trigger and menu. The <select> stays in the DOM
 * as the source of truth: read and write `.value` on it, listen for its `change` event, and call
 * `setSelectValue` (utils/dom.js) to update it from code.
 */
export function initSelect(selectEl) {
  if (!selectEl || selectEl.dataset.selectWired === 'true') return;
  selectEl.dataset.selectWired = 'true';

  const wrap = document.createElement('div');
  wrap.className = 'select-wrap';
  selectEl.parentNode.insertBefore(wrap, selectEl);
  wrap.appendChild(selectEl);
  selectEl.classList.add('select-native');
  selectEl.tabIndex = -1;
  selectEl.setAttribute('aria-hidden', 'true');

  const trigger = document.createElement('button');
  trigger.type = 'button';
  trigger.className = 'select-trigger';
  if (selectEl.classList.contains('compact')) trigger.classList.add('compact');
  trigger.setAttribute('aria-haspopup', 'listbox');
  trigger.setAttribute('aria-expanded', 'false');
  const label = selectEl.getAttribute('aria-label')
    || (selectEl.id && document.querySelector(`label[for="${selectEl.id}"]`)?.textContent.trim());
  if (label) trigger.setAttribute('aria-label', label);
  trigger.innerHTML = `<span class="select-value"></span>${CHEVRON}`;
  wrap.appendChild(trigger);

  // Clicking a <label for=...> focuses the trigger instead of the hidden select.
  if (selectEl.id) {
    document.querySelectorAll(`label[for="${selectEl.id}"]`).forEach(l => {
      l.addEventListener('click', (e) => { e.preventDefault(); trigger.focus(); });
    });
  }

  const valueEl = trigger.querySelector('.select-value');

  function sync() {
    const opt = selectEl.options[selectEl.selectedIndex];
    valueEl.textContent = opt ? opt.textContent : '';
    valueEl.classList.toggle('is-placeholder', !selectEl.value);
    trigger.disabled = selectEl.disabled;
  }

  function open() {
    if (selectEl.disabled) return;
    openMenu(trigger, (menu) => {
      menu.classList.add('select-menu');
      menu.setAttribute('role', 'listbox');
      menu.style.minWidth = `${Math.max(trigger.offsetWidth, 160)}px`;
      [...selectEl.options].forEach(opt => {
        addMenuItem(menu, opt.textContent, () => {
          if (selectEl.value !== opt.value) {
            selectEl.value = opt.value;
            sync();
            selectEl.dispatchEvent(new Event('change', { bubbles: true }));
          }
          trigger.focus({ preventScroll: true });
        }, { selected: opt.value === selectEl.value, disabled: opt.disabled });
      });
    }, { align: 'left' });
  }

  trigger.addEventListener('click', (e) => {
    e.stopPropagation();
    if (trigger.getAttribute('aria-expanded') === 'true') closeMenu();
    else open();
  });

  trigger.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      open();
    }
  });

  selectEl.addEventListener('change', sync);
  selectEl._syncSelect = sync;
  new MutationObserver(sync).observe(selectEl, { childList: true, subtree: true, attributes: true, attributeFilter: ['disabled'] });
  sync();
}

export function initAllSelects(root = document) {
  root.querySelectorAll('select').forEach(initSelect);
}
