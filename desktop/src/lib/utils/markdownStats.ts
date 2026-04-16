/**
 * 변환된 마크다운에서 문서 통계를 뽑는다.
 *
 * 파서 쪽 메타데이터가 불완전해도 (pageCount가 null, etc.) UI에서 바로
 * 쓸 수 있는 수치를 마크다운 문자열만으로 계산한다. 표·이미지는 우리
 * HWPX/HWP 파서의 규약에 맞춰 감지한다:
 *   - 표:    `|`로 시작하는 연속된 라인 블록
 *   - 이미지: `![...`, 또는 내부 placeholder `[이미지: N]`
 *   - 헤딩:   `#`으로 시작하는 라인
 */

export interface DocumentStats {
  /** 마크다운 문자 수 (공백 포함) */
  charCount: number;
  /** 공백 분리 단어 수 (한글은 eojeol 단위) */
  wordCount: number;
  /** 비어 있지 않은 문단 수 */
  paragraphCount: number;
  /** 헤딩 총 개수 */
  headingCount: number;
  /** 표 개수 */
  tableCount: number;
  /** 이미지/플레이스홀더 개수 */
  imageCount: number;
  /** 강조점(<mark>) 블록 수 — 공공문서 핵심 용어 */
  emphasisCount: number;
  /** 취소선 마커 수 */
  strikeoutCount: number;
  /** 체크리스트 항목 수 */
  taskCount: number;
}

export function computeStats(markdown: string): DocumentStats {
  const lines = markdown.split(/\r?\n/);

  let headingCount = 0;
  let tableCount = 0;
  let paragraphCount = 0;
  let taskCount = 0;

  let inTable = false;
  let inParagraph = false;

  for (const raw of lines) {
    const line = raw.trim();

    // Table detection: block of lines starting with '|'
    if (line.startsWith('|') && line.endsWith('|')) {
      if (!inTable) {
        tableCount += 1;
        inTable = true;
      }
      inParagraph = false;
      continue;
    }
    inTable = false;

    if (line === '') {
      inParagraph = false;
      continue;
    }

    // Headings
    if (/^#{1,6}\s/.test(line)) {
      headingCount += 1;
      inParagraph = false;
      continue;
    }

    // Task list items
    if (/^[-*]\s+\[[ xX]\]\s/.test(line)) {
      taskCount += 1;
    }

    if (!inParagraph) {
      paragraphCount += 1;
      inParagraph = true;
    }
  }

  // Global counts via regex on the full document
  const imageCount =
    (markdown.match(/!\[/g)?.length ?? 0) +
    (markdown.match(/\[이미지:\s*[^\]]+\]/g)?.length ?? 0);

  const emphasisCount = markdown.match(/<mark>/g)?.length ?? 0;
  const strikeoutCount = markdown.match(/~~[^~]+~~/g)?.length ?? 0;

  const charCount = markdown.length;
  // 한글 eojeol 기반 word count: 공백으로 분리된 토큰 중 비어있지 않은 것
  const wordCount = markdown
    .split(/\s+/)
    .filter((tok) => tok.length > 0).length;

  return {
    charCount,
    wordCount,
    paragraphCount,
    headingCount,
    tableCount,
    imageCount,
    emphasisCount,
    strikeoutCount,
    taskCount,
  };
}
