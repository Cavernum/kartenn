// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** Types of `gen.mjs`, for the freshness test. */
export declare const CONTRACT: string;
export declare const SCHEMA: string;
export declare const DATED: string;
export declare function readContract(): { paths: Record<string, unknown> } & Record<
  string,
  unknown
>;
export declare function generate(spec: unknown): Promise<{ schema: string; dated: string }>;
