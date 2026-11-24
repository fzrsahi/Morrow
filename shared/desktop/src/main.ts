import { mount } from 'svelte';
import App from './App.svelte';
import { cssVariables } from './lib/tokens';
import './app.css';

for (const [name, value] of Object.entries(cssVariables())) {
  document.documentElement.style.setProperty(name, value);
}

mount(App, { target: document.getElementById('app')! });
