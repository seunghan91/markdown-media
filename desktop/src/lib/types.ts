export type AppMode = 'convert' | 'viewer' | 'batch' | 'export';
export type ViewerMode = 'render' | 'split' | 'source' | 'fidelity';
export type ExportFormat = 'md' | 'docx' | 'hwpx' | 'pdf';

export interface ExtractedImage {
  id: string;
  filename: string;
  mediaType: string;
  width?: number | null;
  height?: number | null;
}

export interface DocumentMetadata {
  format: string;
  title?: string | null;
  author?: string | null;
  subject?: string | null;
  description?: string | null;
  keywords?: string | null;
  version?: string | null;
  pageCount?: number | null;
  wordCount?: number | null;
}

export interface ConvertResult {
  markdown: string;
  images: ExtractedImage[];
  metadata: DocumentMetadata;
}

export interface ViewerData {
  html: string;
  markdown: string;
  metadata: DocumentMetadata;
}

export interface BatchItemResult {
  inputPath: string;
  outputPath?: string | null;
  status: 'success' | 'failed';
  message?: string | null;
}

export interface BatchResult {
  total: number;
  success: number;
  failed: number;
  results: BatchItemResult[];
}

export interface HistoryEntry {
  id: number;
  fileName: string;
  filePath: string;
  direction: 'to_md' | 'from_md';
  outputFormat: string;
  createdAt: string;
  status: 'success' | 'failed';
}
