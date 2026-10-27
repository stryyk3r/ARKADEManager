import { state } from '../../state.js';
import { escapeHtml } from '../../utils/dom.js';
import { refreshPluginDestinations, setupPluginInstall } from './install.js';
import { loadPluginToggleServers, setupPluginToggle } from './toggle.js';
import { asaServerRoot } from './servers.js';

const $ = (id) => document.getElementById(id);

function renderDescription() {
  $('pluginsDescription').innerHTML = state.pluginsTab === 'install'
    ? `ArkApi plugins for servers under <span class="mono">${escapeHtml(asaServerRoot())}</span>`
    : 'Disabled plugin folders carry an <span class="mono">_OFF</span> suffix. Restart the server to apply.';
}

export function setPluginsTab(tab) {
  state.pluginsTab = tab;
  document.querySelectorAll('#plugins [data-plugin-tab]').forEach(t => {
    const on = t.dataset.pluginTab === tab;
    t.classList.toggle('active', on);
    t.setAttribute('aria-selected', String(on));
  });
  $('pluginInstallPanel').classList.toggle('active', tab === 'install');
  $('pluginTogglePanel').classList.toggle('active', tab === 'toggle');
  renderDescription();
  if (tab === 'install') refreshPluginDestinations();
  else loadPluginToggleServers();
}

/** Called when the Plugins page is shown. */
export function showPluginsPage() {
  setPluginsTab(state.pluginsTab);
}

export function setupPluginsPage() {
  document.querySelector('#plugins .tabs').addEventListener('click', (e) => {
    const tab = e.target.closest('[data-plugin-tab]');
    if (tab && tab.dataset.pluginTab !== state.pluginsTab) setPluginsTab(tab.dataset.pluginTab);
  });
  setupPluginInstall();
  setupPluginToggle();
  renderDescription();
}
