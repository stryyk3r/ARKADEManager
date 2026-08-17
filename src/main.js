import './styles/index.css';
import { bindUiActions } from './ui/window-exports.js';
import { initApp } from './ui/init.js';

document.addEventListener('DOMContentLoaded', () => {
  bindUiActions();
  initApp();
});
