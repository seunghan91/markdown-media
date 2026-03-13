/**
 * MDM (Markdown+Media) Parser — TypeScript Declarations
 * @version 0.1.0
 */

/** A tokenized text segment */
export interface TextToken {
  type: 'text';
  value: string;
}

/** A tokenized MDM media reference */
export interface MDMReferenceToken {
  type: 'mdm-reference';
  raw: string;
  name: string;
  preset: string | null;
  attributes: Record<string, string | number | boolean>;
}

export type Token = TextToken | MDMReferenceToken;

/** MDM resource definition (stored in .mdm sidecar files) */
export interface MDMResource {
  type: 'image' | 'video' | 'audio' | 'embed';
  src?: string;
  alt?: string;
  title?: string;
  loading?: 'lazy' | 'eager';
  width?: number | string;
  height?: number | string;
  poster?: string;
  duration?: string;
  /** YouTube / Vimeo etc. */
  provider?: string;
  id?: string;
  /** Per-resource named presets */
  presets?: Record<string, Record<string, unknown>>;
  [key: string]: unknown;
}

/** Parsed .mdm sidecar file */
export interface MDMData {
  version: string;
  media_root?: string;
  /** Named resources */
  resources?: Record<string, MDMResource>;
  /** Global named presets */
  presets?: Record<string, Record<string, unknown>>;
}

/** Options passed to MDMParser.parse() */
export interface ParseOptions {
  /** Path to .mdm sidecar file to load */
  mdmPath?: string;
}

/**
 * Tokenizes MDM-flavored Markdown text into an array of tokens.
 *
 * @example
 * const t = new Tokenizer();
 * const tokens = t.tokenize('![[hero.jpg | width=800 align=center]]');
 */
export declare class Tokenizer {
  /** Find all MDM references and text segments in the given string */
  tokenize(text: string): Token[];
  /** Parse a single `name:preset | attrs` reference string */
  parseReference(reference: string): Omit<MDMReferenceToken, 'type' | 'raw'>;
  /** Parse an attribute string like `width=500 align=center controls` */
  parseAttributes(attributesStr: string): Record<string, string | number | boolean>;
  /** Reconstruct original text from tokens (debugging) */
  reconstruct(tokens: Token[]): string;
}

/**
 * Renders an array of tokens to HTML.
 *
 * @example
 * const r = new Renderer(mdmData);
 * const html = r.render(tokens);
 */
export declare class Renderer {
  constructor(mdmData?: MDMData | null);
  /** Render all tokens to a single HTML string */
  render(tokens: Token[]): string;
  /** Render a single token */
  renderToken(token: Token): string;
  /** Render an MDM reference token using resource lookup and preset resolution */
  renderMDMReference(token: MDMReferenceToken): string;
  /** Render a file by inferring type from its extension */
  renderDirectFile(filename: string, attrs: Record<string, unknown>): string;
}

/**
 * Loads and caches .mdm sidecar files (YAML or JSON).
 */
export declare class MDMLoader {
  /** Load and cache an MDM sidecar file */
  load(mdmPath: string): Promise<MDMData>;
  /** Parse YAML or JSON content */
  parse(content: string, ext: string): MDMData;
  /** Validate an MDM data object */
  validate(data: MDMData): void;
  /** Resolve relative resource paths against the sidecar file's directory */
  normalizePaths(data: MDMData, basePath: string): MDMData;
  /** Clear the in-memory cache */
  clearCache(): void;
}

/**
 * Full MDM parsing pipeline: load sidecar → tokenize → render → HTML.
 *
 * @example
 * const parser = new MDMParser();
 * await parser.loadMDM('./my-blog.mdm');
 * const html = await parser.parse(markdownString);
 */
export declare class MDMParser {
  constructor(options?: { mdmPath?: string });
  /** Load a .mdm sidecar file and prepare the renderer */
  loadMDM(mdmPath: string): Promise<MDMData>;
  /**
   * Parse MDM-flavored Markdown to HTML.
   * If `options.mdmPath` is provided and no MDM data is loaded yet, loads it first.
   */
  parse(markdown: string, options?: ParseOptions): Promise<string>;
  /** Return raw tokens (useful for debugging or custom renderers) */
  tokenize(markdown: string): Token[];
  /** Directly inject MDM data without loading a file */
  setMDMData(data: MDMData): void;
  /** Return the currently loaded MDM data */
  getMDMData(): MDMData | null;
  /** Clear the loader cache and reset loaded MDM data */
  clearCache(): void;
}

/**
 * Convenience function — create a one-shot parser and render MDM Markdown to HTML.
 *
 * @example
 * const html = await parse('![[hero.jpg | width=800]]');
 */
export declare function parse(markdown: string, options?: ParseOptions): Promise<string>;
