import '@fontsource/manrope/latin-400.css';
import '@fontsource/manrope/latin-500.css';
import '@fontsource/manrope/latin-600.css';
import '@fontsource/manrope/latin-700.css';
import '@fontsource/manrope/latin-800.css';
import '@fontsource/jetbrains-mono/latin-400.css';
import '@fontsource/jetbrains-mono/latin-500.css';
import '@fontsource/jetbrains-mono/latin-600.css';
import './styles/index.css';
import { bindUiActions } from './ui/window-exports.js';
import { initApp } from './ui/init.js';

document.addEventListener('DOMContentLoaded', () => {
  bindUiActions();
  initApp();
});
