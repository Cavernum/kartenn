import { INTENT_DELAY, prefetch, type Prefetchable } from "@kartenn/sdk";

// The children of a closed node, read on intent (REQ-021-22; design « Préchargement »): the
// pointer held 60 ms on its row, or the focus resting there as long (the arrows of the
// keyboard pass over many rows). One listener on the tree, for every row, rendered or
// not yet: the rows of `TreeGrid` carry `data-node` and `aria-expanded`.

const CLOSED = '[role="row"][data-node][aria-expanded="false"]';

/**
 * Prefetches `children(id)` for the closed node under the pointer or the focus, inside
 * `root`. Returns the function that stops listening.
 */
export function childrenIntent(
  root: HTMLElement,
  children: (id: string) => readonly Prefetchable[],
  delay: number = INTENT_DELAY,
): () => void {
  // The pointer and the focus each have their own row and delay: one leaving the tree
  // does not cancel the other.
  function rest() {
    let row: Element | null = null;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const stop = () => {
      if (timer !== null) clearTimeout(timer);
      timer = null;
      row = null;
    };
    const enter = (event: Event) => {
      const next = event.target instanceof Element ? event.target.closest(CLOSED) : null;
      // Moving within the same row keeps the delay running.
      if (next !== null && next === row) return;
      stop();
      const id = next !== null && root.contains(next) ? next.getAttribute("data-node") : null;
      if (id === null) return;
      row = next;
      timer = setTimeout(() => {
        timer = null;
        void prefetch.prefetch(children(id));
      }, delay);
    };
    return { enter, stop };
  }
  const pointer = rest();
  const focus = rest();
  root.addEventListener("pointerover", pointer.enter);
  root.addEventListener("pointerleave", pointer.stop);
  root.addEventListener("focusin", focus.enter);
  root.addEventListener("focusout", focus.stop);
  return () => {
    pointer.stop();
    focus.stop();
    root.removeEventListener("pointerover", pointer.enter);
    root.removeEventListener("pointerleave", pointer.stop);
    root.removeEventListener("focusin", focus.enter);
    root.removeEventListener("focusout", focus.stop);
  };
}
