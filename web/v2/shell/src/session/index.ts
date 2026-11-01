import { ApiProblem } from "@kartenn/sdk";
import { ref, type Ref } from "vue";
import {
  DENSITY_KEY,
  MOTION_KEY,
  PANEL_KEY,
  SIDEBAR_KEY,
  SINGLE_KEYS_KEY,
  THEME_KEY,
} from "../frame/settings.ts";
import { LOCALE_KEY } from "../i18n/index.ts";
import { forgetRecent } from "../shortcuts/recent.ts";

// The session of the new shell (REQ-021-14, REQ-007-28; design 007 « Session »): login by
// an issuer or a break-glass account, back to the same URL — dates and filters of its query
// included — after a login, a 401 in use that offers to log in again, a logout that clears
// what is sensitive before it leaves. The drafts stay at the server (024): nothing of them
// is kept here.

/** The URL to come back to after a login, in `sessionStorage` (one tab). */
export const RETURN_KEY = "kartenn.return";
/** The last issuer chosen, in `localStorage`: « Se reconnecter » goes straight to it. */
export const ISSUER_KEY = "kartenn.issuer";
/**
 * What a logout keeps: the language, the settings of display and of the keyboard (theme,
 * density, motion, single keys, sidebar, panel) and the last issuer. Everything else under
 * `kartenn.` goes, the recent objects of the palette first.
 */
export const KEPT_KEYS: readonly string[] = [
  LOCALE_KEY,
  THEME_KEY,
  DENSITY_KEY,
  MOTION_KEY,
  SINGLE_KEYS_KEY,
  SIDEBAR_KEY,
  PANEL_KEY,
  ISSUER_KEY,
];

function store(kind: "localStorage" | "sessionStorage"): Storage | null {
  try {
    return globalThis[kind] ?? null;
  } catch {
    return null;
  }
}

/**
 * An internal path of the shell, else `null`: a return URL never leaves the origin
 * (`//host`, `/\host` and absolute URLs refused) nor goes back to the login page or the API.
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

/**
 * Where the login page goes once logged in: the `return` of its URL, which wins over a URL
 * kept from an older login, else that one, else the home page. The one kept is read and
 * dropped either way: it never sends a later navigation elsewhere.
 */
export function returnAfterLogin(explicit: unknown): string {
  const kept = takeReturn();
  return internalPath(explicit) ?? kept ?? "/";
}

/** The issuer chosen last, if any. */
export function lastIssuer(): string | null {
  return store("localStorage")?.getItem(ISSUER_KEY) ?? null;
}

/** The address that starts a login at `issuer` (REQ-006-24). */
export function loginUrl(issuer: string): string {
  return `/v1/auth/login?issuer=${encodeURIComponent(issuer)}`;
}

/** Leaves for the issuer, the return URL and the issuer kept. */
export function loginAt(
  issuer: string,
  back: unknown,
  go: (url: string) => void = (url) => location.assign(url),
): void {
  rememberReturn(back);
  store("localStorage")?.setItem(ISSUER_KEY, issuer);
  go(loginUrl(issuer));
}

/**
 * Clears what a logout must not leave behind (REQ-007-28, 021-14): the recent objects of
 * every user of the browser (`forgetRecent`), every `kartenn.*` key of `localStorage` but
 * those `KEPT_KEYS` names, and every `kartenn.*` key of the tab.
 */
export function clearSensitive(): void {
  forgetRecent();
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
}

/**
 * The logout (design 007 « Session »): what is sensitive goes first, then a top-level form
 * `POST /v1/auth/logout`, which follows the redirection to the issuer's end of session.
 */
export function logout(submit: (form: HTMLFormElement) => void = (form) => form.submit()): void {
  try {
    clearSensitive();
  } catch (error) {
    // A storage that refuses must not keep the session open: the logout goes on.
    console.warn("kartenn: local data not cleared", error);
  }
  const form = document.createElement("form");
  form.method = "post";
  form.action = "/v1/auth/logout";
  form.hidden = true;
  document.body.append(form);
  submit(form);
}

/**
 * The issuers of the login page (`GET /v1/auth/issuers`, public), by their identifier;
 * the problem of the answer when they cannot be read (its type and `Request-Id` shown).
 * Read outside the SDK's client: signed out, a 401 is no expired session.
 */
export async function readIssuers(fetcher: typeof fetch = fetch): Promise<string[] | ApiProblem> {
  const response = await fetcher("/v1/auth/issuers").catch(() => null);
  if (!response) return new ApiProblem({ status: 0, type: "about:blank", title: "network" });
  if (!response.ok) return ApiProblem.from(response);
  const body = (await response.json().catch(() => null)) as { id?: unknown }[] | null;
  return Array.isArray(body)
    ? body.flatMap((issuer) => (typeof issuer.id === "string" ? [issuer.id] : []))
    : new ApiProblem({ status: response.status, type: "about:blank", title: "not a list" });
}

/** The proofs of a break-glass account (REQ-005, `POST /v1/auth/breakglass`). */
export interface Breakglass {
  name: string;
  passphrase: string;
  code: string;
}

/**
 * Opens a break-glass session; `null` once the cookie is set, else the problem of the
 * answer (its `type` and `Request-Id` shown), or of the network.
 */
export async function breakglassLogin(
  proofs: Breakglass,
  fetcher: typeof fetch = fetch,
): Promise<ApiProblem | null> {
  const response = await fetcher("/v1/auth/breakglass", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(proofs),
  }).catch(() => null);
  if (response?.ok) return null;
  return response
    ? ApiProblem.from(response)
    : new ApiProblem({ status: 0, type: "about:blank", title: "network" });
}

/** True while a 401 waits for the user to log in again (« Session expirée »). */
export const expired: Ref<boolean> = ref(false);
