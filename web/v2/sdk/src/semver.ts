// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The `sdk` range of a plugin manifest against the version of the SDK (W7): `^x[.y[.z]]`,
// `~x.y[.z]` or an exact `x.y.z`, with the meaning of the npm ranges. Precedence follows
// Semantic Versioning 2.0.0 §11; a prerelease satisfies a range only if the range names a
// prerelease of the same `x.y.z` (it is never chosen by a range of releases).

interface Version {
  major: number;
  minor: number;
  patch: number;
  pre: (string | number)[];
}

const NUMBER = "(0|[1-9]\\d*)";
const PRE = "(?:-([0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*))?";
const BUILD = "(?:\\+[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*)?";
const VERSION = new RegExp(`^${NUMBER}\\.${NUMBER}\\.${NUMBER}${PRE}${BUILD}$`);
const RANGE = new RegExp(`^([\\^~]?)${NUMBER}(?:\\.${NUMBER}(?:\\.${NUMBER}${PRE})?)?$`);

/** A version, or `null` if it is not semver 2.0.0. */
export function parseVersion(text: string): Version | null {
  const m = VERSION.exec(text);
  if (!m) return null;
  return {
    major: Number(m[1]),
    minor: Number(m[2]),
    patch: Number(m[3]),
    pre: parsePre(m[4]),
  };
}

function parsePre(text: string | undefined): (string | number)[] {
  if (!text) return [];
  return text.split(".").map((id) => (/^(0|[1-9]\d*)$/.test(id) ? Number(id) : id));
}

/** Precedence of two versions (§11): negative, zero or positive. */
export function compare(a: Version, b: Version): number {
  const core = a.major - b.major || a.minor - b.minor || a.patch - b.patch;
  if (core !== 0) return core;
  if (a.pre.length === 0 || b.pre.length === 0) return b.pre.length - a.pre.length;
  for (let i = 0; i < Math.max(a.pre.length, b.pre.length); i++) {
    const x = a.pre[i];
    const y = b.pre[i];
    if (x === undefined) return -1;
    if (y === undefined) return 1;
    if (x === y) continue;
    if (typeof x === "number" && typeof y === "number") return x - y;
    if (typeof x === "number") return -1;
    if (typeof y === "number") return 1;
    return x < y ? -1 : 1;
  }
  return 0;
}

/** Whether `version` satisfies `range`; `false` for anything malformed. */
export function satisfies(version: string, range: string): boolean {
  const v = parseVersion(version);
  const m = RANGE.exec(range.trim());
  if (!v || !m) return false;
  const [, op, majorText, minorText, patchText] = m;
  const major = Number(majorText);
  const minor = minorText === undefined ? 0 : Number(minorText);
  const patch = patchText === undefined ? 0 : Number(patchText);
  const low: Version = { major, minor, patch, pre: parsePre(m[5]) };

  if (op === "") {
    // Exact: all three numbers, build metadata ignored.
    return patchText !== undefined && compare(v, low) === 0;
  }
  if (op === "~" && minorText === undefined) return false;

  let high: Version;
  if (op === "~") high = { major, minor: minor + 1, patch: 0, pre: [] };
  else if (major > 0 || minorText === undefined)
    high = { major: major + 1, minor: 0, patch: 0, pre: [] };
  else if (minor > 0 || patchText === undefined)
    high = { major: 0, minor: minor + 1, patch: 0, pre: [] };
  else high = { major: 0, minor: 0, patch: patch + 1, pre: [] };

  if (v.pre.length > 0) {
    const sameTuple = v.major === low.major && v.minor === low.minor && v.patch === low.patch;
    if (!(sameTuple && low.pre.length > 0)) return false;
  }
  // The upper bound excludes the prereleases of `high` too (`<high-0`).
  return compare(v, low) >= 0 && compare({ ...v, pre: [] }, high) < 0;
}

/** The ranges a plugin manifest declares (`ui_plugins`; spec 021, REQ-021-26). */
export interface Ranges {
  sdk: string;
  /** Absent for a plugin that does not import `@kartenn/ui`. */
  ui?: string | null;
  /** Absent for a plugin without the shared framework. */
  framework?: string | null;
}

/** The versions of the modules the shell shares through its import map (ADR 0038). */
export interface SharedVersions {
  sdk: string;
  ui: string;
  framework: string;
}

/**
 * The first range of a plugin the shell does not satisfy, or `null`: such a plugin is set
 * aside `incompatible` (REQ-007-9, REQ-021-26). A range the plugin does not declare is
 * not checked.
 */
export function unmetRange(ranges: Ranges, versions: SharedVersions): keyof SharedVersions | null {
  for (const name of ["sdk", "ui", "framework"] as const) {
    const range = ranges[name];
    if (range != null && !satisfies(versions[name], range)) return name;
  }
  return null;
}
