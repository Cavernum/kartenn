// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * The design tokens of the shell (REQ-007-30): CSS custom properties, inherited through
 * shadow roots, so that a plugin writes `var(--k-color-bg)` whatever its framework. The
 * shell's `styles/tokens.css` defines each of them, in light and dark modes.
 */
export const tokens = Object.freeze([
  "--k-color-bg",
  "--k-color-surface",
  "--k-color-text",
  "--k-color-muted",
  "--k-color-accent",
  "--k-color-danger",
  "--k-color-warning",
  "--k-color-border",
  "--k-color-focus",
  "--k-space-1",
  "--k-space-2",
  "--k-space-3",
  "--k-space-4",
  "--k-radius",
  "--k-font-family",
  "--k-font-mono",
  "--k-font-size",
  "--k-row-height",
] as const) satisfies readonly `--k-${string}`[];

export type Token = (typeof tokens)[number];
