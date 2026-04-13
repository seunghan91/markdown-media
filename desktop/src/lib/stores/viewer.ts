import { browser } from '$app/environment';
import { writable } from 'svelte/store';
import type { ViewerData, ViewerMode } from '$lib/types';

const STORAGE_KEY = 'mdm-desktop.viewer-mode';

const initialMode: ViewerMode = browser
  ? ((window.localStorage.getItem(STORAGE_KEY) as ViewerMode | null) ?? 'render')
  : 'render';

export const viewerMode = writable<ViewerMode>(initialMode);
export const viewerData = writable<ViewerData | null>(null);
export const viewerPath = writable<string | null>(null);

viewerMode.subscribe((mode) => {
  if (browser) {
    window.localStorage.setItem(STORAGE_KEY, mode);
  }
});

export function setViewerMode(mode: ViewerMode) {
  viewerMode.set(mode);
}

export function cycleViewerMode() {
  viewerMode.update((mode) => {
    if (mode === 'render') return 'split';
    if (mode === 'split') return 'source';
    return 'render';
  });
}
