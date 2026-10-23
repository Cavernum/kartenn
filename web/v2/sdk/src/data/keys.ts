// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The keys of the cache (ADR 0037, decision 2; design « Clés et fraîcheur »):
// `['v1', resource, path parameters, normalized filters, { valid_at, as_of }]`, the present
// written `null`. No screen writes a key by hand: these factories do.

import { DATED_PATHS } from "../api/dated.ts";
import { context } from "../context.ts";
import { collectionOf } from "./types.ts";

/** The dated context of a key: `null` for the present. */
export interface TimeKey {
  readonly valid_at: string | null;
  readonly as_of: string | null;
}

/** A dated context as the SDK writes it (`context.get().time`). */
export interface TimeArg {
  readonly validAt?: string | null;
  readonly asOf?: string | null;
}

/** The value of a filter: repeated when an array; absent when `null`, `undefined` or `""`. */
export type FilterValue =
  string | number | boolean | null | undefined | readonly (string | number | boolean)[];
export type Filters = { readonly [name: string]: FilterValue };
/** Filters as a key holds them: names sorted, values as text, repeated values sorted. */
export type NormalFilters = { readonly [name: string]: string | readonly string[] };
export type PathParams = { readonly [name: string]: string };

/** The key of an object: `['v1', type, id, null, time]`. */
export type ObjectKey = readonly ["v1", string, string, null, TimeKey];
/** The key of a read by path, a list or another resource: `['v1', path, params, filters, time]`. */
export type PathKey = readonly ["v1", string, PathParams | null, NormalFilters, TimeKey];
/** The key of a sub-resource of an object: `['v1', type, id, sub, params, time]`. */
export type SubKey = readonly ["v1", string, string, string, NormalFilters, TimeKey];
export type Key = ObjectKey | PathKey | SubKey;

const PRESENT: TimeKey = Object.freeze({ valid_at: null, as_of: null });

/** The dated part of a key; the present, `{ valid_at: null, as_of: null }`. */
export function timeKey(time?: TimeArg | null): TimeKey {
  const validAt = time?.validAt ?? null;
  const asOf = time?.asOf ?? null;
  return validAt === null && asOf === null
    ? PRESENT
    : Object.freeze({ valid_at: validAt, as_of: asOf });
}

/** The time of a key on a path: the context by default, the present on an undated path. */
function timeOn(template: string, time: TimeArg | null | undefined): TimeKey {
  if (!DATED_PATHS.has(template)) return PRESENT;
  return timeKey(time === undefined ? context.get().time : time);
}

/** Filters in a canonical form, so that two equal queries get the same key. */
export function normalize(filters?: Filters | null): NormalFilters {
  const out: Record<string, string | readonly string[]> = {};
  for (const name of Object.keys(filters ?? {}).sort()) {
    const value = filters![name];
    if (value === undefined || value === null || value === "") continue;
    if (Array.isArray(value)) {
      const values = [...new Set(value.map(String).filter((v) => v !== ""))].sort();
      if (values.length > 0) out[name] = Object.freeze(values);
    } else {
      out[name] = String(value);
    }
  }
  return Object.freeze(out);
}

function collection(type: string): string {
  const path = collectionOf(type);
  if (path === null) throw new TypeError(`unknown type ${type}: registerType() first`);
  return path;
}

/** Whether a dated read takes time from the key (the object route is dated). */
function objectTemplate(type: string): string {
  return `${collection(type)}/{id}`;
}

export const keys = {
  /** An object by its type and identifier. */
  object(type: string, id: string, time?: TimeArg | null): ObjectKey {
    return Object.freeze(["v1", type, id, null, timeOn(objectTemplate(type), time)] as const);
  },
  /** A list, or any read by path (`/v1/me`, `/v1/capabilities`), with its filters. */
  list(
    path: string,
    filters?: Filters | null,
    time?: TimeArg | null,
    params?: PathParams,
  ): PathKey {
    return Object.freeze([
      "v1",
      path,
      params ? Object.freeze({ ...params }) : null,
      normalize(filters),
      timeOn(path, time),
    ] as const);
  },
  /** A sub-resource of an object: `children`, `occupancy`, `history`, `properties`… */
  sub(
    type: string,
    id: string,
    sub: string,
    params?: Filters | null,
    time?: TimeArg | null,
  ): SubKey {
    const template = `${collection(type)}/{id}/${sub}`;
    return Object.freeze(["v1", type, id, sub, normalize(params), timeOn(template, time)] as const);
  },
  /** Every key of an object: itself and its sub-resources, at every date. */
  allOf(type: string, id: string): readonly ["v1", string, string] {
    return ["v1", type, id];
  },
  /** Every key of a path: its lists at every date and filter. */
  allOn(path: string): readonly ["v1", string] {
    return ["v1", path];
  },
};

/** Whether a key reads the past as recorded (`as_of`): immutable (ADR 0005). */
export function isImmutable(key: readonly unknown[]): boolean {
  const time = key[key.length - 1] as TimeKey | undefined;
  return key[0] === "v1" && typeof time === "object" && time !== null && time.as_of != null;
}

/** Whether a key reads the present. */
export function isPresent(key: readonly unknown[]): boolean {
  const time = key[key.length - 1] as TimeKey | undefined;
  return time?.valid_at === null && time.as_of === null;
}

/** Whether a key is that of an object (`keys.object`). */
export function isObjectKey(key: readonly unknown[]): key is ObjectKey {
  return key.length === 5 && key[0] === "v1" && typeof key[2] === "string" && key[3] === null;
}

/** Whether a key is that of a sub-resource (`keys.sub`). */
export function isSubKey(key: readonly unknown[]): key is SubKey {
  return key.length === 6 && key[0] === "v1" && typeof key[3] === "string";
}
