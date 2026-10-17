/// <reference types="vite/client" />

declare const __LEARNVAULT_TRANSPORT__: 'http' | 'desktop';

declare module '*.svelte' {
	import type { Component } from 'svelte';
	const component: Component;
	export default component;
}
