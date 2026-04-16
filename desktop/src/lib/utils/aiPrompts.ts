/**
 * Preset prompts for shipping extracted markdown to an external LLM.
 *
 * v1 keeps things simple: we build a prompt string (instruction +
 * document body) and copy it to the clipboard, then open the
 * provider's chat URL in a new window so the user can paste. No API
 * keys, no secrets, no server-side proxy — just get the context into
 * the right chat as fast as possible.
 *
 * A future v2 could replace `openInChat` with a direct Anthropic /
 * OpenAI / Google call using a key stored in the OS keyring via the
 * Tauri keyring plugin. Schema below already separates instruction
 * from document so the same template feeds either path.
 */

export type LlmProvider = 'claude' | 'chatgpt' | 'gemini' | 'perplexity';
export type PromptPreset = 'summary' | 'translate' | 'qa' | 'rewrite';

interface ProviderInfo {
  label: string;
  url: string;
}

const PROVIDERS: Record<LlmProvider, ProviderInfo> = {
  claude: { label: 'Claude', url: 'https://claude.ai/new' },
  chatgpt: { label: 'ChatGPT', url: 'https://chatgpt.com/' },
  gemini: { label: 'Gemini', url: 'https://gemini.google.com/app' },
  perplexity: { label: 'Perplexity', url: 'https://www.perplexity.ai/' },
};

interface PresetInfo {
  label: string;
  instruction: string;
}

const PRESETS: Record<PromptPreset, PresetInfo> = {
  summary: {
    label: '요약',
    instruction:
      '다음 문서를 한국어로 3문장 이내로 요약해주세요. 핵심 수치·날짜·인명이 있다면 보존하세요.',
  },
  translate: {
    label: '영어 번역',
    instruction:
      'Translate the following Korean document to English. Preserve all numbers, dates, proper nouns, and the original structure (headings, lists, tables).',
  },
  qa: {
    label: 'Q&A 준비',
    instruction:
      '이 문서를 기반으로 질의응답을 하려고 합니다. 주요 내용을 읽었다고 답해주시고, 질문을 기다려 주세요. 출처는 이 문서로 한정합니다.',
  },
  rewrite: {
    label: '평문으로 다듬기',
    instruction:
      '이 공공 문서의 내용을 일반 독자가 이해하기 쉬운 한국어로 다듬어 주세요. 전문 용어는 처음 등장할 때 괄호로 뜻을 달아주세요. 팩트는 바꾸지 말고, 구조는 원본의 섹션을 따라가세요.',
  },
};

export function listProviders(): Array<{ id: LlmProvider; label: string }> {
  return (Object.keys(PROVIDERS) as LlmProvider[]).map((id) => ({
    id,
    label: PROVIDERS[id].label,
  }));
}

export function listPresets(): Array<{ id: PromptPreset; label: string }> {
  return (Object.keys(PRESETS) as PromptPreset[]).map((id) => ({
    id,
    label: PRESETS[id].label,
  }));
}

export function buildPrompt(preset: PromptPreset, markdown: string): string {
  const { instruction } = PRESETS[preset];
  return `${instruction}\n\n---\n\n${markdown}`;
}

export function providerUrl(provider: LlmProvider): string {
  return PROVIDERS[provider].url;
}

/**
 * Open the provider's chat interface in a new window/tab.
 * Works in both browser fallback (window.open) and Tauri (external
 * URL via the shell plugin when available, else window.open).
 */
export async function openInChat(provider: LlmProvider): Promise<void> {
  const url = providerUrl(provider);
  try {
    // Prefer Tauri shell if available — respects the OS's default browser.
    const maybeTauri = (window as unknown as { __TAURI_INTERNALS__?: unknown })
      .__TAURI_INTERNALS__;
    if (maybeTauri) {
      const { open } = await import('@tauri-apps/plugin-shell');
      await open(url);
      return;
    }
  } catch {
    // Fall through to window.open
  }
  window.open(url, '_blank', 'noopener,noreferrer');
}
