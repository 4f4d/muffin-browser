import { mount } from 'svelte';
import App from './routes/App.svelte';
import './style.css';
import { installChromeBridge } from './lib/bridge';

installChromeBridge();
mount(App, { target: document.getElementById('app')! });
