import { mount } from 'svelte';
import SettingsApp from './SettingsApp.svelte';
import { cssVariables } from './lib/tokens';
import './app.css';
import './settings.css';

for (const [name, value] of Object.entries(cssVariables())) {
  document.documentElement.style.setProperty(name, value);
}
if ('__TAURI_INTERNALS__' in window) document.body.classList.add('tauri');

mount(SettingsApp, { target: document.getElementById('app')! });
