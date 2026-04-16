/**
 * Link two scrollable elements so their scroll positions track each
 * other proportionally. When the user scrolls either element, the other
 * moves to the same fractional position.
 *
 * This is a crude approximation of the "block ID" mapping proposed in
 * RFC 001 (docs/rfcs/001-rhwp-bridge.md) — once the bridge gives us
 * per-paragraph anchors we can replace the proportional ratio with a
 * proper nearest-block jump. For the near-term split view between the
 * rendered markdown and the source textarea, proportional sync is good
 * enough: both panes render the same content at roughly the same line
 * density, so the mismatch stays small.
 *
 * Guards against feedback loops with a single-tick flag: while we're
 * mirroring a scroll event we ignore the corresponding scroll events
 * fired by the target element.
 *
 * Returns a dispose function — call it from `onDestroy` or when the
 * split mode turns off.
 */
export function linkScroll(a: HTMLElement | null, b: HTMLElement | null): () => void {
  if (!a || !b) return () => {};

  let syncing = false;

  function scrollRatio(el: HTMLElement): number {
    const range = el.scrollHeight - el.clientHeight;
    return range > 0 ? el.scrollTop / range : 0;
  }

  function applyRatio(el: HTMLElement, ratio: number): void {
    const range = el.scrollHeight - el.clientHeight;
    el.scrollTop = ratio * range;
  }

  function makeHandler(source: HTMLElement, target: HTMLElement) {
    return () => {
      if (syncing) return;
      syncing = true;
      applyRatio(target, scrollRatio(source));
      // Release the lock after the browser has processed the mirror —
      // requestAnimationFrame is enough for the scroll event loop to
      // settle without us also mirroring the echo.
      requestAnimationFrame(() => {
        syncing = false;
      });
    };
  }

  const aHandler = makeHandler(a, b);
  const bHandler = makeHandler(b, a);

  a.addEventListener('scroll', aHandler, { passive: true });
  b.addEventListener('scroll', bHandler, { passive: true });

  return () => {
    a.removeEventListener('scroll', aHandler);
    b.removeEventListener('scroll', bHandler);
  };
}
