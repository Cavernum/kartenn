// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The context offered to the extensions (REQ-007-14, 24): the user, the dated context and
// the language, held once by the single instance of the SDK and pushed by the shell.

/** Types of objects; modules add their own. */
export type ObjectType =
  "entity" | "site" | "domain" | "prefix" | "range" | "address" | (string & {});

/** The object an extension is shown for: its `object` property. */
export interface ObjectRef {
  type: ObjectType;
  id: string;
  href: string;
}

/** The dated context of the URL (W9): `present` when neither date is set. */
export interface TimeContext {
  validAt: string | null;
  asOf: string | null;
  present: boolean;
}

/**
 * The user, as `/v1/me` sums them up. `has` is an aid to show or hide: the server stays
 * the only judge (design « Droits dans l'interface »).
 */
export interface User {
  identity: string;
  name: string | null;
  kind: string;
  has(permission: string): boolean;
}

export type Locale = "fr" | "en";

export interface Context {
  user: User;
  time: TimeContext;
  locale: Locale;
}

const ANONYMOUS: User = Object.freeze({
  identity: "",
  name: null,
  kind: "anonymous",
  has: () => false,
});

let current: Readonly<Context> = Object.freeze({
  user: ANONYMOUS,
  time: Object.freeze({ validAt: null, asOf: null, present: true }),
  locale: "en",
});

const listeners = new Set<(c: Readonly<Context>) => void>();

/** The context, read at any time or followed by its changes. */
export const context = {
  get(): Readonly<Context> {
    return current;
  },
  /** Called at each change; returns the function that stops listening. */
  subscribe(listener: (c: Readonly<Context>) => void): () => void {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
};

/** What the shell pushes (`@kartenn/sdk/host`): the members that change. */
export interface ContextUpdate {
  user?: User;
  time?: { validAt: string | null; asOf: string | null };
  locale?: Locale;
}

/** Replaces the changed members and notifies the listeners, once per update. */
export function setContext(update: ContextUpdate): void {
  const time = update.time
    ? Object.freeze({
        validAt: update.time.validAt,
        asOf: update.time.asOf,
        present: update.time.validAt === null && update.time.asOf === null,
      })
    : current.time;
  current = Object.freeze({
    user: update.user ? Object.freeze({ ...update.user }) : current.user,
    time,
    locale: update.locale ?? current.locale,
  });
  for (const listener of [...listeners]) listener(current);
}

/** Back to the context of a signed-out user (logout, tests). */
export function resetContext(): void {
  setContext({ user: ANONYMOUS, time: { validAt: null, asOf: null }, locale: "en" });
}
