import { browser } from '$app/environment';
import { goto } from '$app/navigation';
import { writable } from 'svelte/store';
import type { ConvertResult, ViewerData, ViewerMode } from '$lib/types';
import { markdownToHtml } from '$lib/utils/ipc';

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

/** ConvertResult를 ViewerData로 변환 후 스토어에 세팅하고 뷰어로 이동 */
export async function openInViewer(result: ConvertResult, filePath?: string) {
  const html = await markdownToHtml(result.markdown);
  viewerData.set({ markdown: result.markdown, html, metadata: result.metadata });
  if (filePath) viewerPath.set(filePath);
  goto('/viewer');
}
