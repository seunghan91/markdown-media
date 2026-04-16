/**
 * Sidecar notes store for the viewer.
 *
 * MDM is a one-way pipeline: the original HWP is never modified. To give
 * users a place to annotate extracted documents without touching the
 * source, we store notes in a separate JSON "sidecar" keyed by the
 * document's path (or its title when no path is available).
 *
 * v1 backend: localStorage. This is deliberately simple — notes survive
 * reload within the same desktop app install, and the JSON schema is
 * portable. A future patch should migrate to a `.mdm.notes.json` file
 * next to the source via the Tauri fs plugin; the shape below is
 * designed to round-trip straight to disk.
 */

export interface SidecarNote {
  id: string;
  /** Optional line number (0-based) in the extracted markdown. */
  line?: number;
  text: string;
  /** ISO-8601 UTC. */
  createdAt: string;
}

export interface SidecarNotes {
  schemaVersion: 1;
  notes: SidecarNote[];
}

const STORAGE_PREFIX = 'mdm.notes.v1:';

function storageKey(documentKey: string): string {
  return STORAGE_PREFIX + documentKey;
}

export function loadNotes(documentKey: string): SidecarNotes {
  if (typeof localStorage === 'undefined') {
    return { schemaVersion: 1, notes: [] };
  }
  const raw = localStorage.getItem(storageKey(documentKey));
  if (!raw) return { schemaVersion: 1, notes: [] };
  try {
    const parsed = JSON.parse(raw) as SidecarNotes;
    if (parsed.schemaVersion === 1 && Array.isArray(parsed.notes)) {
      return parsed;
    }
  } catch {
    // Corrupt entry — fall through to empty.
  }
  return { schemaVersion: 1, notes: [] };
}

export function saveNotes(documentKey: string, sidecar: SidecarNotes): void {
  if (typeof localStorage === 'undefined') return;
  localStorage.setItem(storageKey(documentKey), JSON.stringify(sidecar));
}

export function addNote(
  sidecar: SidecarNotes,
  text: string,
  line?: number
): SidecarNotes {
  const trimmed = text.trim();
  if (!trimmed) return sidecar;
  const note: SidecarNote = {
    id: crypto.randomUUID(),
    line,
    text: trimmed,
    createdAt: new Date().toISOString(),
  };
  return {
    ...sidecar,
    notes: [...sidecar.notes, note].sort((a, b) => {
      const la = a.line ?? Number.POSITIVE_INFINITY;
      const lb = b.line ?? Number.POSITIVE_INFINITY;
      if (la !== lb) return la - lb;
      return a.createdAt.localeCompare(b.createdAt);
    }),
  };
}

export function deleteNote(sidecar: SidecarNotes, id: string): SidecarNotes {
  return { ...sidecar, notes: sidecar.notes.filter((n) => n.id !== id) };
}

/**
 * Export the sidecar as a pretty-printed JSON string suitable for
 * downloading as `<basename>.mdm.notes.json`.
 */
export function exportSidecar(sidecar: SidecarNotes): string {
  return JSON.stringify(sidecar, null, 2);
}
