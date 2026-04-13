import { writable } from 'svelte/store';
import type { HistoryEntry } from '$lib/types';
import { getHistory } from '$lib/utils/ipc';

export const historyEntries = writable<HistoryEntry[]>([]);
export const historyLoading = writable(false);
export const historyError = writable<string | null>(null);

export async function refreshHistory(limit = 12) {
  historyLoading.set(true);
  historyError.set(null);

  try {
    const entries = await getHistory(limit);
    historyEntries.set(entries);
  } catch (error) {
    historyError.set(error instanceof Error ? error.message : '히스토리를 불러오지 못했습니다.');
  } finally {
    historyLoading.set(false);
  }
}
