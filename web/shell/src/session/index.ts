import { onUnauthorized } from "@kartenn/sdk/host";
import { ref, type Ref } from "vue";
import type { Router } from "vue-router";
import { STORAGE_KEY as LOCALE_KEY } from "../i18n/index.ts";
import { loadSession, useSession, type Session } from "../rights/index.ts";
import { THEME_KEY } from "../theme.ts";

// The session of the shell (REQ-007-28, design « Session »): login by an issuer or a
// break-glass account, back to the same URL after a login, a 401 that offers to log in
// again, a logout that clears what is sensitive before it leaves.

/** The URL to come back to after a login, in `sessionStorage` (one tab). */
export const RETURN_KEY = "kartenn.return";
/** The last issuer chosen, in `localStorage`: « Se reconnecter » goes straight to it. */
export const ISSUER_KEY = "kartenn.issuer";
/** What a logout keeps: language, theme, last issuer. */
export const KEPT_KEYS: readonly string[] = [LOCALE_KEY, THEME_KEY, ISSUER_KEY];

/** Routes open without a session. */
export const PUBLIC_ROUTES: readonly string[] = ["login", "about"];

function store(kind: "localStorage" | "sessionStorage"): Storage | null {
  try {
    return globalThis[kind] ?? null;
  } catch {
    return null;
  }
}

/**
 * An internal path of the shell, else `null`: a return URL never leaves the origin
 * (`//host`, `/\host` and absolute URLs refused) nor goes back to the login page.
 */
export function internalPath(value: unknown): string | null {
  if (typeof value !== "string" || !value.startsWith("/")) return null;
  if (value.startsWith("//") || value.startsWith("/\\")) return null;
  if (value === "/login" || value.startsWith("/login?") || value.startsWith("/v1/")) return null;
  return value;
}

/** Keeps the URL to come back to after the next login. */
export function rememberReturn(path: unknown): void {
  const internal = internalPath(path);
  if (internal) store("sessionStorage")?.setItem(RETURN_KEY, internal);
}

/** The URL kept for after the login, read once. */
export function takeReturn(): string | null {
  const storage = store("sessionStorage");
  const value = internalPath(storage?.getItem(RETURN_KEY) ?? null);
  storage?.removeItem(RETURN_KEY);
  return value;
}

export function lastIssuer(): string | null {
  return store("localStorage")?.getItem(ISSUER_KEY) ?? null;
}

/** The address that starts a login at `issuer` (REQ-006-24). */
export function loginUrl(issuer: string): string {
  return `/v1/auth/login?issuer=${encodeURIComponent(issuer)}`;
}

/** Leaves for the issuer, the return URL and the issuer kept. */
export function loginAt(issuer: string, back: unknown, go = (url: string) => location.assign(url)) {
  rememberReturn(back);
  store("localStorage")?.setItem(ISSUER_KEY, issuer);
  go(loginUrl(issuer));
}

/**
 * Clears what a logout must not leave behind (REQ-007-28): every `kartenn.*` key but the
 * language, the theme and the last issuer, the tab's keys, and the local databases of the
 * shell (the drafts, W10).
 */
export async function clearSensitive(): Promise<void> {
  const local = store("localStorage");
  if (local) {
    const keys = Array.from({ length: local.length }, (_, i) => local.key(i)).filter(
      (key): key is string =>
        key !== null && key.startsWith("kartenn.") && !KEPT_KEYS.includes(key),
    );
    for (const key of keys) local.removeItem(key);
  }
  const tab = store("sessionStorage");
  if (tab) {
    const keys = Array.from({ length: tab.length }, (_, i) => tab.key(i)).filter(
      (key): key is string => key !== null && key.startsWith("kartenn."),
    );
    for (const key of keys) tab.removeItem(key);
  }
  const databases = globalThis.indexedDB;
  if (databases && typeof databases.databases === "function") {
    const names = await databases.databases().catch(() => []);
    for (const { name } of names) {
      if (name?.startsWith("kartenn")) databases.deleteDatabase(name);
    }
  }
}

/** True while a 401 waits for the user to log in again. */
export const expired: Ref<boolean> = ref(false);

/** A `fetch` of the shell's own reads: a 401 opens the dialog « Session expirée ». */
export async function shellFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const response = await fetch(input, init);
  if (response.status === 401) expired.value = true;
  return response;
}

/**
 * Installs the session on the router: the client's 401 opens the dialog; a page that needs
 * a session sends to `/login` when there is none; the first page after a login goes back
 * to the URL kept.
 */
export function installSession(
  router: Router,
  load: () => Promise<unknown> = loadSession,
  current: () => Session = () => useSession().value,
): void {
  onUnauthorized(() => {
    expired.value = true;
  });
  // The first navigation waits for `/v1/me`; the next ones read the session as it is
  // (a break-glass login reloads it in place).
  let first: Promise<unknown> | null = null;
  router.beforeEach(async (to) => {
    first ??= load();
    await first;
    if (!current().me) {
      if (PUBLIC_ROUTES.includes(String(to.name))) return true;
      return { name: "login", query: { return: to.fullPath } };
    }
    const back = takeReturn();
    return back && back !== to.fullPath ? back : true;
  });
}
