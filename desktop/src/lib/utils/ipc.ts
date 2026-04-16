import type {
  BatchResult,
  ConvertResult,
  ExportFormat,
  HistoryEntry,
  ViewerData
} from '$lib/types';

function isTauriEnvironment() {
  return typeof window !== 'undefined' && typeof window.__TAURI_INTERNALS__ !== 'undefined';
}

async function invokeCommand<T>(command: string, payload: Record<string, unknown>) {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(command, payload);
}

const mockHistory: HistoryEntry[] = [
  {
    id: 1,
    fileName: '정부보고서.docx',
    filePath: '/mock/정부보고서.docx',
    direction: 'to_md',
    outputFormat: 'md',
    createdAt: '2026-04-13T11:20:00+09:00',
    status: 'success'
  },
  {
    id: 2,
    fileName: '회의록.md',
    filePath: '/mock/회의록.md',
    direction: 'from_md',
    outputFormat: 'docx',
    createdAt: '2026-04-13T10:42:00+09:00',
    status: 'success'
  }
];

export async function markdownToHtml(markdown: string) {
  if (isTauriEnvironment()) {
    return invokeCommand<string>('convert_text', {
      content: markdown,
      fromFormat: 'markdown'
    });
  }

  const escaped = markdown
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');

  return escaped
    .split(/\n{2,}/)
    .map((block) => {
      if (block.startsWith('### ')) return `<h3>${block.slice(4)}</h3>`;
      if (block.startsWith('## ')) return `<h2>${block.slice(3)}</h2>`;
      if (block.startsWith('# ')) return `<h1>${block.slice(2)}</h1>`;
      return `<p>${block.replace(/\n/g, '<br />')}</p>`;
    })
    .join('');
}

/**
 * Tauri 네이티브 파일 열기 다이얼로그. 실제 파일 시스템 경로를 반환.
 * 브라우저 모드에서는 null 반환.
 */
export async function pickFileWithDialog(options?: {
  title?: string;
  filters?: { name: string; extensions: string[] }[];
  multiple?: boolean;
  directory?: boolean;
}): Promise<string | string[] | null> {
  if (!isTauriEnvironment()) return null;

  const { open } = await import('@tauri-apps/plugin-dialog');
  return open({
    title: options?.title ?? '파일 선택',
    multiple: options?.multiple ?? false,
    directory: options?.directory ?? false,
    filters: options?.filters ?? [
      { name: '문서 파일', extensions: ['hwp', 'hwpx', 'pdf', 'docx', 'md', 'mdx', 'markdown'] }
    ],
  });
}

export async function convertFile(path: string, format = 'markdown') {
  if (isTauriEnvironment()) {
    return invokeCommand<ConvertResult>('convert_file', { path, format });
  }

  return {
    markdown: `# ${path.split('/').pop()}\n\n브라우저 프리뷰 모드입니다.\n\n- 실제 변환은 Tauri 런타임에서 동작합니다.\n- 현재는 UI 검증용 더미 데이터가 표시됩니다.`,
    images: [],
    metadata: {
      format,
      title: path.split('/').pop() ?? '샘플 문서',
      author: 'MDM Desktop',
      pageCount: 1,
      version: 'preview'
    }
  } satisfies ConvertResult;
}

export async function openFile(path: string) {
  if (isTauriEnvironment()) {
    return invokeCommand<ViewerData>('open_file', { path });
  }

  const converted = await convertFile(path, 'markdown');
  return {
    markdown: converted.markdown,
    html: await markdownToHtml(converted.markdown),
    metadata: converted.metadata
  } satisfies ViewerData;
}

export async function getHistory(limit = 12) {
  if (isTauriEnvironment()) {
    return invokeCommand<HistoryEntry[]>('get_history', { limit });
  }

  return mockHistory.slice(0, limit);
}

export async function batchConvert(paths: string[], format: ExportFormat, outputDir: string) {
  if (isTauriEnvironment()) {
    return invokeCommand<BatchResult>('batch_convert', { paths, format, outputDir });
  }

  return {
    total: paths.length,
    success: paths.length,
    failed: 0,
    results: paths.map((path) => ({
      inputPath: path,
      outputPath: `${outputDir}/${path.split('/').pop()}.${format}`,
      status: 'success',
      message: 'preview'
    }))
  } satisfies BatchResult;
}

// ─── rhwp (vendored) HWP edit bridge ─────────────────────────────────────────

export interface RhwpParagraph {
  section: number;
  index: number;
  text: string;
  charCount: number;
}

export interface RhwpParagraphEdit {
  section: number;
  index: number;
  newText: string;
}

export interface RhwpSaveSummary {
  sourceBytes: number;
  outputBytes: number;
  paragraphsEdited: number;
  outputPath: string;
}

/**
 * Parse a .hwp / .hwpx file via the vendored rhwp crate and return a flat
 * list of top-level paragraphs. Browser fallback returns an empty list —
 * edit functionality is native-only.
 */
export async function rhwpListParagraphs(path: string): Promise<RhwpParagraph[]> {
  if (!isTauriEnvironment()) return [];
  return invokeCommand<RhwpParagraph[]>('rhwp_list_paragraphs', { path });
}

/**
 * Round-trip a HWP through rhwp's parse/serialize, optionally applying
 * simple text replacements per paragraph. Returns size and count metadata
 * for the caller to surface.
 */
export async function rhwpSaveWithEdits(
  sourcePath: string,
  targetPath: string,
  edits: RhwpParagraphEdit[]
): Promise<RhwpSaveSummary> {
  if (!isTauriEnvironment()) {
    throw new Error('HWP 편집 저장은 데스크톱(Tauri) 환경에서만 동작합니다.');
  }
  return invokeCommand<RhwpSaveSummary>('rhwp_save_with_edits', {
    sourcePath,
    targetPath,
    edits: edits.map((e) => ({ section: e.section, index: e.index, new_text: e.newText })),
  });
}

export async function exportMarkdown(
  markdown: string,
  format: Exclude<ExportFormat, 'md'>,
  template: string,
  output: string
) {
  if (isTauriEnvironment()) {
    if (format === 'docx') {
      return invokeCommand<void>('export_to_docx', { markdown, template, output });
    }

    if (format === 'hwpx') {
      return invokeCommand<void>('export_to_hwpx', { markdown, template, output });
    }

    return invokeCommand<void>('export_to_pdf', { markdown, output });
  }
}
