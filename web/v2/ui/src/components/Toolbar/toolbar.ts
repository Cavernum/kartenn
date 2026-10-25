// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName } from "../../icons/index.ts";
import type { ButtonVariant } from "../Button/button.ts";
import { CONTAINER_WIDTHS } from "../layout.ts";

/** An action of a toolbar (design « Inventaire », line 33; REQ-020-36). */
export interface ToolbarAction {
  id: string;
  label: string;
  icon?: IconName;
  /** The icon alone in the bar, the label as its name (and in the « … » menu). */
  iconOnly?: boolean;
  variant?: ButtonVariant;
  /** The higher, the later it folds; equal priorities fold from the end. 0 by default. */
  priority?: number;
  shortcut?: string[];
  disabled?: boolean;
  /** Why it is refused (a policy), its tooltip. */
  reason?: string;
  /** The permission the user lacks (024/M14). */
  forbidden?: string;
}

/**
 * The main action of a bar: the primary one, else the one of highest priority (the first
 * among equals).
 */
export function mainAction(actions: readonly ToolbarAction[]): ToolbarAction | undefined {
  return (
    actions.find((a) => a.variant === "primary") ??
    actions.reduce<ToolbarAction | undefined>(
      (best, a) => (!best || (a.priority ?? 0) > (best.priority ?? 0) ? a : best),
      undefined,
    )
  );
}

/**
 * The actions to fold so that the others fit in `available` pixels (REQ-020-36): none if
 * all fit; else, from the lowest priority and from the end, until the rest and the « … »
 * button (`more` pixels) fit. `widths` are the measured widths, `gap` the space between
 * two items. Under `narrow` pixels (a container `xs`, design « Adaptation à la largeur »),
 * a bar that must fold keeps its main action alone.
 */
export function foldActions(
  actions: readonly ToolbarAction[],
  widths: ReadonlyMap<string, number>,
  available: number,
  gap: number,
  more: number,
  narrow: number = CONTAINER_WIDTHS.xs,
): string[] {
  const width = (ids: readonly string[]) =>
    ids.reduce((sum, id) => sum + (widths.get(id) ?? 0), 0) + gap * Math.max(0, ids.length - 1);
  const all = actions.map((a) => a.id);
  if (width(all) <= available) return [];
  if (available < narrow) {
    const main = mainAction(actions)?.id;
    return all.filter((id) => id !== main);
  }
  const order = actions
    .map((action, index) => ({ action, index }))
    .sort((a, b) => (a.action.priority ?? 0) - (b.action.priority ?? 0) || b.index - a.index)
    .map(({ action }) => action.id);
  const folded: string[] = [];
  for (const id of order) {
    folded.push(id);
    const kept = all.filter((a) => !folded.includes(a));
    if (width(kept) + (kept.length ? gap : 0) + more <= available) break;
  }
  return folded;
}
