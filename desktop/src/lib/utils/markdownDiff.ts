/**
 * Line-level diff for comparing two markdown outputs.
 *
 * Target use case: 신구대조표 (legal before/after) — comparing two HWP files
 * that were converted to markdown. Line granularity is the right unit here:
 * paragraphs, table rows, and headings all sit on distinct lines in our
 * extractor output.
 *
 * Uses LCS (Longest Common Subsequence) to find matching lines, then walks
 * back to emit a unified op stream. O(N*M) time/space — fine for documents
 * up to ~5k lines. Beyond that we'd switch to a Myers patience diff, but
 * legal circulars sit comfortably under that ceiling.
 */

export type DiffOp =
  | { kind: 'equal'; left: string; right: string; leftIndex: number; rightIndex: number }
  | { kind: 'delete'; left: string; leftIndex: number }
  | { kind: 'insert'; right: string; rightIndex: number };

export interface DiffStats {
  equal: number;
  added: number;
  removed: number;
  totalLeft: number;
  totalRight: number;
}

export interface DiffResult {
  ops: DiffOp[];
  stats: DiffStats;
}

export function diffLines(leftText: string, rightText: string): DiffResult {
  const a = leftText.split(/\r?\n/);
  const b = rightText.split(/\r?\n/);
  const n = a.length;
  const m = b.length;

  // LCS length table — allocate (n+1) × (m+1) flattened for locality.
  const lcs = new Uint32Array((n + 1) * (m + 1));
  const idx = (i: number, j: number) => i * (m + 1) + j;

  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      if (a[i] === b[j]) {
        lcs[idx(i, j)] = lcs[idx(i + 1, j + 1)] + 1;
      } else {
        const below = lcs[idx(i + 1, j)];
        const right = lcs[idx(i, j + 1)];
        lcs[idx(i, j)] = below > right ? below : right;
      }
    }
  }

  const ops: DiffOp[] = [];
  let i = 0;
  let j = 0;
  let equal = 0;
  let added = 0;
  let removed = 0;

  while (i < n && j < m) {
    if (a[i] === b[j]) {
      ops.push({ kind: 'equal', left: a[i], right: b[j], leftIndex: i, rightIndex: j });
      equal++;
      i++;
      j++;
    } else if (lcs[idx(i + 1, j)] >= lcs[idx(i, j + 1)]) {
      ops.push({ kind: 'delete', left: a[i], leftIndex: i });
      removed++;
      i++;
    } else {
      ops.push({ kind: 'insert', right: b[j], rightIndex: j });
      added++;
      j++;
    }
  }
  while (i < n) {
    ops.push({ kind: 'delete', left: a[i], leftIndex: i });
    removed++;
    i++;
  }
  while (j < m) {
    ops.push({ kind: 'insert', right: b[j], rightIndex: j });
    added++;
    j++;
  }

  return {
    ops,
    stats: { equal, added, removed, totalLeft: n, totalRight: m },
  };
}
