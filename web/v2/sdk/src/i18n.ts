// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Translation for plugins (REQ-007-29, W11). The shell plugs its own translator (vue-i18n,
// catalogs of plugins merged under `<name>.`); without it, as in the test sandbox, a
// plain lookup in catalogs given here, with `{param}` interpolation.

import { context } from "./context.ts";

/** A catalog: nested keys, `{param}` placeholders. */
export type Messages = { [key: string]: string | Messages };
type Translator = (key: string, params?: Record<string, unknown>) => string;

const catalogs = new Map<string, Messages>();

function lookup(messages: Messages | undefined, key: string): string | null {
  let node: string | Messages | undefined = messages;
  for (const part of key.split(".")) {
    if (typeof node !== "object") return null;
    node = node[part];
  }
  return typeof node === "string" ? node : null;
}

function interpolate(text: string, params: Record<string, unknown> = {}): string {
  return text.replace(/\{(\w+)\}/g, (all, name: string) =>
    name in params ? String(params[name]) : all,
  );
}

const plain: Translator = (key, params) => {
  const { locale } = context.get();
  const text = lookup(catalogs.get(locale), key) ?? lookup(catalogs.get("en"), key);
  return text === null ? key : interpolate(text, params);
};

let translator: Translator = plain;

/** The full key (`<plugin>.<key>`) in the language of the context. */
export function translate(key: string, params?: Record<string, unknown>): string {
  return translator(key, params);
}

/** The shell's translator, or `null` for the plain lookup. */
export function setTranslator(next: Translator | null): void {
  translator = next ?? plain;
}

/** Adds a catalog of a plugin, under its name, for the plain lookup. */
export function addCatalog(locale: string, namespace: string, messages: Messages): void {
  const all = catalogs.get(locale) ?? {};
  all[namespace] = messages;
  catalogs.set(locale, all);
}

/** Formats in the language of the context and the time zone of the browser, named. */
export const format = {
  dateTime(iso: string): string {
    return new Intl.DateTimeFormat(context.get().locale, {
      dateStyle: "medium",
      timeStyle: "medium",
    }).format(new Date(iso));
  },
  number(n: number): string {
    return new Intl.NumberFormat(context.get().locale).format(n);
  },
  get timeZone(): string {
    return Intl.DateTimeFormat().resolvedOptions().timeZone;
  },
};
