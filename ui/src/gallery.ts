import { mount } from 'svelte';
import Gallery from './routes/Gallery.svelte';
import './gallery.css';

mount(Gallery, { target: document.getElementById('app')! });
