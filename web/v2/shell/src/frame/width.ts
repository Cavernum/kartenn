import { WIDTH_CLASSES, widthQuery, type WidthClass } from "@kartenn/ui";
import { readonly, ref, type Ref } from "vue";

// The width class of the frame (spec 020, « Adaptation à la largeur »), published for the
// gate of writes (`time/index.ts`, `useWrites`; REQ-021-29, Q-021-12): the frame alone reads
// the window, by the queries of its sheet (`WIDTH_CLASSES`), and follows it while it is
// mounted (`followWindow`); every other part reads the class here, never the window. Without
// a frame (a mock-up of the catalogue, a test), the class stays `wide`.

const width = ref<WidthClass>("wide");

/** The width class of the frame, reactive, read-only. */
export const frameWidth: Readonly<Ref<WidthClass>> = readonly(width);

/**
 * The frame's: the class follows the window from now on; the function returned stops it
 * (the class then stays the last one read).
 */
export function followWindow(): () => void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return () => {};
  const queries = WIDTH_CLASSES.map((c) => ({
    id: c.id,
    list: window.matchMedia(widthQuery(c.id)),
  }));
  const read = () => (width.value = queries.find((q) => q.list.matches)?.id ?? "wide");
  read();
  for (const q of queries) q.list.addEventListener("change", read);
  return () => {
    for (const q of queries) q.list.removeEventListener("change", read);
  };
}

/** Tests only: the class as if the window were of it. */
export function setFrameWidth(value: WidthClass): void {
  width.value = value;
}
