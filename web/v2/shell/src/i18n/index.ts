import { setContext, setTranslator } from "@kartenn/sdk/host";
import { setLocale as setLibraryLocale } from "@kartenn/ui";
import { createI18n } from "vue-i18n";
import changesetsEn from "../changesets/i18n/en.json";
import changesetsFr from "../changesets/i18n/fr.json";
import viewsEn from "../views/i18n/shared/en.json";
import viewsFr from "../views/i18n/shared/fr.json";
import en from "./en.json";
import fr from "./fr.json";

// i18n of the shell (REQ-007-29, W11; design 021 « Pile retenue »): vue-i18n, catalogs FR
// and EN (keys taken from the old shell where they exist), the language kept locally, else
// the browser's, else English. One change of language reaches the shell, @kartenn/ui
// (`setLocale`), the SDK's context and `<html lang>`.

export const LOCALES = ["fr", "en"] as const;
export type Locale = (typeof LOCALES)[number];
export const LOCALE_KEY = "kartenn.locale";
/** Each language named in itself, as the user menu offers it. */
export const LOCALE_NAMES: Record<Locale, string> = { fr: "Français", en: "English" };

function storage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

/** The language: the one kept, else the first of the browser's that is offered, else `en`. */
export function chooseLocale(stored: string | null, languages: readonly string[]): Locale {
  if (stored && (LOCALES as readonly string[]).includes(stored)) return stored as Locale;
  for (const language of languages) {
    const prefix = language.toLowerCase().split("-")[0];
    if ((LOCALES as readonly string[]).includes(prefix ?? "")) return prefix as Locale;
  }
  return "en";
}

/** Sets `locale` everywhere but in vue-i18n: the library, the SDK, the document. */
function spread(locale: Locale): void {
  setLibraryLocale(locale);
  setContext({ locale });
  document.documentElement.lang = locale;
}

/**
 * The plugins whose catalogs the shell holds, under `plugin.<name>` (task 16): apart from
 * the shell's own keys, so that a plugin named `app` never rewrites the shell's words.
 */
const namespaces = new Set<string>();

/**
 * The key the shell looks up for a key of the SDK: under `plugin.` only a key translated
 * for a plugin whose catalogs it holds (the `t` of its handle, `plugin` given); the SDK's
 * own keys (`labels.<space>.<key>`) never, whatever plugin bears that name.
 */
export function shellKey(key: string, plugin?: string): string {
  return plugin !== undefined && namespaces.has(plugin) && key.startsWith(`${plugin}.`)
    ? `plugin.${key}`
    : key;
}

/**
 * Merges a catalog of a plugin into `i18n` under `plugin.<name>`, and sends the keys of
 * that plugin's `t` there.
 */
export function mergePluginCatalog(
  i18n: { global: { mergeLocaleMessage(locale: string, messages: object): void } },
  locale: string,
  name: string,
  messages: object,
): void {
  i18n.global.mergeLocaleMessage(locale, { plugin: { [name]: messages } });
  namespaces.add(name);
}

/** A catalog the shell reads with the screens that use it, in each of its languages. */
export type Catalog = Record<Locale, object>;

type Merging = { global: { mergeLocaleMessage(locale: string, messages: object): void } };

/** The i18n instance of the shell, once created: the catalogs read later merge into it. */
let shell: Merging | undefined;
/** The catalogs read on demand so far, merged again into an instance created after them. */
const read: Catalog[] = [];

function mergeCatalog(i18n: Merging, catalog: Catalog): void {
  for (const locale of LOCALES) i18n.global.mergeLocaleMessage(locale, catalog[locale]);
}

/**
 * A catalog kept out of the initial shell (task 22h, budget « Shell initial »): `load` at
 * the first call, its words merged deep into the shell's, the screens that use it awaiting
 * it with their own chunk; a load that fails is tried again at the next call.
 */
export function lazyCatalog(load: () => Promise<{ default: Catalog }>): () => Promise<void> {
  let loading: Promise<void> | undefined;
  return () =>
    (loading ??= load().then(
      ({ default: catalog }) => {
        read.push(catalog);
        if (shell) mergeCatalog(shell, catalog);
      },
      (error: unknown) => {
        loading = undefined;
        throw error;
      },
    ));
}

/** The i18n instance of the shell, plugged into the SDK (`t` of plugins, context). */
export function createShellI18n(
  locale: Locale = chooseLocale(
    storage()?.getItem(LOCALE_KEY) ?? null,
    globalThis.navigator?.languages ?? [],
  ),
) {
  const i18n = createI18n({
    legacy: false,
    locale,
    fallbackLocale: "en",
    // The screens of the referential keep their words apart (`views`, `labels`, task 11):
    // here those the frame reads, the others with the screens (`views/words.ts`, task 22n).
    messages: { fr: { ...fr, ...viewsFr }, en: { ...en, ...viewsEn } },
    missingWarn: false,
    fallbackWarn: false,
  });
  // The changesets and the drafts (task 14), merged deep: they add words under `views` too.
  i18n.global.mergeLocaleMessage("fr", changesetsFr);
  i18n.global.mergeLocaleMessage("en", changesetsEn);
  // The catalogs read on demand (the screens', `views/words.ts`; the administration's,
  // `admin/routes.ts`), here and later.
  for (const catalog of read) mergeCatalog(i18n, catalog);
  shell = i18n;
  setTranslator((key, params, plugin) => {
    const full = shellKey(key, plugin);
    const text = i18n.global.t(full, params ?? {});
    // A key the catalogs lack comes back as the plugin wrote it, never under `plugin.`.
    return text === full ? key : text;
  });
  spread(locale);
  return i18n;
}

/** Changes the language of a composer (`useI18n()` or `i18n.global`), and keeps it. */
export function changeLocale(composer: { locale: { value: string } }, locale: Locale): void {
  composer.locale.value = locale;
  storage()?.setItem(LOCALE_KEY, locale);
  spread(locale);
}
