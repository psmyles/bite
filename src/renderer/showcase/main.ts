import { mount } from 'svelte';
import Showcase from './Showcase.svelte';
import '../assets/fonts.css';
import '../assets/theme.css';

mount(Showcase, { target: document.getElementById('app')! });
