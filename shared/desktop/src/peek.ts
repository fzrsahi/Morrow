import { mount } from 'svelte';
import AnchoringPeek from './lib/components/AnchoringPeek.svelte';
import { cssVariables } from './lib/tokens';
import './app.css';

for (const [name, value] of Object.entries(cssVariables())) {
  document.documentElement.style.setProperty(name, value);
}
if ('__TAURI_INTERNALS__' in window) document.body.classList.add('tauri');

mount(AnchoringPeek, { target: document.getElementById('app')! });
