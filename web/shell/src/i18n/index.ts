import type { ApiProblem } from "@kartenn/sdk";
import { setContext, setTranslator } from "@kartenn/sdk/host";
import { createI18n } from "vue-i18n";
import en from "./en.json";
import fr from "./fr.json";

// i18n of the shell (REQ-007-29, W11): vue-i18n, catalogs FR and EN, the language kept
// locally, else the browser's, else English; plugins' catalogs merged under their name.

export const LOCALES = ["fr", "en"] as const;
export type Locale = (typeof LOCALES)[number];
export const STORAGE_KEY = "kartenn.locale";

/** The language: the one kept, else the first of the browser's that is offered, else `en`. */
export function chooseLocale(stored: string | null, languages: readonly string[]): Locale {
  if (stored && (LOCALES as readonly string[]).includes(stored)) return stored as Locale;
  for (const language of languages) {
    const prefix = language.toLowerCase().split("-")[0];
    if ((LOCALES as readonly string[]).includes(prefix ?? "")) return prefix as Locale;
  }
  return "en";
}

type Lookup = { t: (key: string) => string; te: (key: string) => boolean };

/**
 * The text of a problem of the API: `problem.<slug>` for `urn:kartenn:problem:<slug>`, else
 * its `detail`, else its `title`.
 */
export function problemText(
  i18n: Lookup,
  problem: Pick<ApiProblem, "type" | "detail" | "title">,
): string {
  const slug = /^urn:kartenn:problem:([a-z0-9-]+)$/.exec(problem.type)?.[1];
  if (slug && i18n.te(`problem.${slug}`)) return i18n.t(`problem.${slug}`);
  return problem.detail ?? problem.title;
}

function storage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

/** The i18n instance of the shell, plugged into the SDK (`t` of plugins, context). */
export function createShellI18n(
  locale: Locale = chooseLocale(
    storage()?.getItem(STORAGE_KEY) ?? null,
    globalThis.navigator?.languages ?? [],
  ),
) {
  const i18n = createI18n({
    legacy: false,
    locale,
    fallbackLocale: "en",
    messages: { fr, en },
    missingWarn: false,
    fallbackWarn: false,
  });
  setTranslator((key, params) => i18n.global.t(key, params ?? {}));
  setContext({ locale });
  document.documentElement.lang = locale;
  return i18n;
}

/**
 * Changes the language of a composer (`useI18n()`, global scope, or `i18n.global`): kept
 * locally, pushed to the SDK, set on the document.
 */
export function setLocale(composer: { locale: { value: string } }, locale: Locale): void {
  composer.locale.value = locale;
  storage()?.setItem(STORAGE_KEY, locale);
  setContext({ locale });
  document.documentElement.lang = locale;
}
