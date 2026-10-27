// One floating menu at a time, positioned against its anchor and kept inside the viewport.

let current = null;

export function isMenuOpen() {
  return current !== null;
}

export function closeMenu() {
  if (!current) return false;
  const { menu, anchor, onClose } = current;
  current = null;
  menu.remove();
  anchor?.setAttribute('aria-expanded', 'false');
  onClose?.();
  return true;
}

export function positionMenu(anchor, menu, align = 'right') {
  menu.style.visibility = 'hidden';
  menu.style.top = '0px';
  menu.style.left = '0px';

  const rect = anchor.getBoundingClientRect();
  const gap = 4;
  const height = menu.offsetHeight;
  const width = menu.offsetWidth;

  const spaceBelow = window.innerHeight - rect.bottom - gap;
  const spaceAbove = rect.top - gap;
  const openBelow = spaceBelow >= height || spaceBelow >= spaceAbove;
  const top = openBelow ? rect.bottom + gap : rect.top - gap - height;

  let left = align === 'left' ? rect.left : rect.right - width;
  left = Math.max(8, Math.min(left, window.innerWidth - width - 8));

  menu.style.top = `${Math.max(8, top)}px`;
  menu.style.left = `${left}px`;
  menu.style.visibility = '';
}

/**
 * Open a menu under `anchor`. `build(menu)` fills it (use addMenuItem / addMenuDivider).
 * Returns the menu element.
 */
export function openMenu(anchor, build, { align = 'right', className = '', minWidth, onClose, focus = true } = {}) {
  closeMenu();
  const menu = document.createElement('div');
  menu.className = `menu ${className}`.trim();
  menu.setAttribute('role', 'menu');
  if (minWidth) menu.style.minWidth = `${minWidth}px`;
  build(menu);
  document.body.appendChild(menu);
  positionMenu(anchor, menu, align);
  anchor.setAttribute('aria-expanded', 'true');
  current = { menu, anchor, onClose };
  if (focus) {
    const target = menu.querySelector('.menu-item.is-selected') || menu.querySelector('.menu-item:not(:disabled)');
    target?.focus({ preventScroll: true });
  }
  return menu;
}

export function toggleMenu(anchor, build, options) {
  if (current && current.anchor === anchor) {
    closeMenu();
    return null;
  }
  return openMenu(anchor, build, options);
}

export function addMenuItem(menu, label, onSelect, { danger = false, selected = false, html = null, disabled = false } = {}) {
  const item = document.createElement('button');
  item.type = 'button';
  item.className = 'menu-item';
  item.setAttribute('role', 'menuitem');
  if (danger) item.classList.add('danger');
  if (selected) item.classList.add('is-selected');
  if (html !== null) item.innerHTML = html;
  else item.textContent = label;
  item.disabled = disabled;
  item.addEventListener('click', (e) => {
    e.stopPropagation();
    closeMenu();
    onSelect?.();
  });
  menu.appendChild(item);
  return item;
}

export function addMenuDivider(menu) {
  const div = document.createElement('div');
  div.className = 'menu-divider';
  div.setAttribute('role', 'separator');
  menu.appendChild(div);
}

function moveFocus(step) {
  const items = [...current.menu.querySelectorAll('.menu-item:not(:disabled)')];
  if (items.length === 0) return;
  const index = items.indexOf(document.activeElement);
  const next = index === -1 ? (step > 0 ? 0 : items.length - 1) : (index + step + items.length) % items.length;
  items[next].focus({ preventScroll: true });
}

document.addEventListener('mousedown', (e) => {
  if (!current) return;
  if (current.menu.contains(e.target) || current.anchor.contains(e.target)) return;
  closeMenu();
}, true);

document.addEventListener('keydown', (e) => {
  if (!current) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopImmediatePropagation();
    const anchor = current.anchor;
    closeMenu();
    anchor?.focus({ preventScroll: true });
  } else if (e.key === 'ArrowDown') {
    e.preventDefault();
    moveFocus(1);
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    moveFocus(-1);
  } else if (e.key === 'Tab') {
    closeMenu();
  }
}, true);

document.addEventListener('scroll', (e) => {
  if (current && !current.menu.contains(e.target)) closeMenu();
}, true);

window.addEventListener('resize', () => closeMenu());
