import { writable } from 'svelte/store';
import type { AppMode } from '$lib/types';

export const appMode = writable<AppMode>('convert');
export const sidebarCollapsed = writable(false);

export function setMode(mode: AppMode) {
  appMode.set(mode);
}

export function toggleSidebar() {
  sidebarCollapsed.update((collapsed) => !collapsed);
}
