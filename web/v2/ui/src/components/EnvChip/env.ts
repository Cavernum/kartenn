// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** A hue of the closed palette of environments, `--k-env-1` to `--k-env-6` (REQ-020-8). */
export type EnvHue = 1 | 2 | 3 | 4 | 5 | 6;

const encoder = new TextEncoder();

/**
 * The hue of an environment, read from its key (design « Environnements », decided on
 * 2026-10-23): FNV-1a in 32 bits over the UTF-8 bytes of the key, modulo 6, plus 1 — the
 * same for every user, every session and every implementation. A choice of the
 * administrator, later, takes precedence (the `hue` of `EnvChip`).
 */
export function envHue(key: string): EnvHue {
  let print = 0x811c9dc5;
  for (const byte of encoder.encode(key)) {
    print ^= byte;
    print = Math.imul(print, 0x01000193) >>> 0;
  }
  return ((print % 6) + 1) as EnvHue;
}
