import type { components } from "@kartenn/sdk";
import { nextTick, watch, type Ref } from "vue";

/** An open anomaly, as `GET /v1/anomalies` lists it (006, REQ-002-36). */
export type Anomaly = components["schemas"]["AnomalyDto"];

/**
 * The focus after the last anomaly of a list is acknowledged (2.4.3): the list read again
 * is empty, the grid replaced by the empty state, its row and the button that opened the
 * dialog gone with it, and the focus would fall to the body. Armed by the acknowledgement,
 * the next rows read, if empty, put it on `target` (the empty state), unless it is
 * somewhere already. The grid keeps the focus itself while rows remain (`AnomalyGrid`).
 */
export function useFocusWhenEmptied(
  rows: () => readonly unknown[],
  target: Readonly<Ref<HTMLElement | null>>,
): { arm: () => void } {
  let armed = false;
  watch(rows, async (now) => {
    if (!armed) return;
    armed = false;
    if (now.length > 0) return;
    await nextTick();
    const active = document.activeElement;
    if (active && active !== document.body) return;
    target.value?.focus();
  });
  return { arm: () => (armed = true) };
}
