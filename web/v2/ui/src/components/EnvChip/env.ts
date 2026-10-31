// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** A hue of the closed palette of environments, `--k-env-1` to `--k-env-6` (REQ-020-8). */
export type EnvHue = 1 | 2 | 3 | 4 | 5 | 6;

/**
 * The hue of an environment, read from its key (design « Environnements », decided on
 * 2026-10-23): a stable print of the key (FNV-1a, 32 bits, over its UTF-16 code units)
 * modulo 6, the same for every user and every session. A choice of the administrator,
 * later, takes precedence (the `hue` of `EnvChip`).
 */
export function envHue(key: string): EnvHue {
  let print = 0x811c9dc5;
  for (let i = 0; i < key.length; i++) {
    print ^= key.charCodeAt(i);
    print = Math.imul(print, 0x01000193) >>> 0;
  }
  return ((print % 6) + 1) as EnvHue;
}
