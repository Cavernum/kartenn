import { api, type components } from "@kartenn/sdk";
import { setContext } from "@kartenn/sdk/host";
import { shallowRef, type ShallowRef } from "vue";
import { NO_RIGHTS, Rights } from "./permissions.ts";

export { covers, NO_RIGHTS, Rights } from "./permissions.ts";

// The session of the shell as the frame reads it (design 007 « Droits dans l'interface »,
// repris par la 021): `GET /v1/me` once before the first page of the frame, the rights it
// gives as an aid to show or hide. The server stays the only judge. Login, the dialog of
// an expired session and logout are task 16's.

/** The caller as `GET /v1/me` sums them up (contract 006, 025). */
export type Me = components["schemas"]["Me"];

export interface Session {
  me: Me | null;
  rights: Rights;
}

const state: ShallowRef<Session> = shallowRef({ me: null, rights: NO_RIGHTS });

/** What the session reads of the API: the `get` of the SDK's client. */
export type Reader = (url: string) => Promise<unknown>;

const read: Reader = async (url) => (await api.request("GET", url)).data;

/**
 * The entries of a caller: those of their global bindings, then those of the roles of
 * their scoped bindings, read by `GET /v1/roles/{key}` when the caller may (otherwise only
 * the server knows them: hidden here, never refused).
 */
export async function rightsOf(me: Me, get: Reader = read): Promise<Rights> {
  const scoped = new Set(me.bindings.filter((b) => b.scope.kind !== "global").map((b) => b.role));
  const entries = [...me.global_permissions];
  await Promise.all(
    [...scoped].map(async (key) => {
      const role = (await get(`/v1/roles/${encodeURIComponent(key)}`).catch(() => null)) as {
        entries?: string[];
      } | null;
      entries.push(...(role?.entries ?? []));
    }),
  );
  return new Rights(entries);
}

/**
 * Reads `/v1/me` again; signed out on 401 or when the request fails. Pushes the user to
 * the SDK's context.
 */
export async function loadSession(get: Reader = read): Promise<Session> {
  const me = ((await get("/v1/me").catch(() => null)) as Me | null) ?? null;
  const rights = me ? await rightsOf(me, get) : NO_RIGHTS;
  state.value = { me, rights };
  if (me) {
    setContext({
      user: {
        identity: me.identity,
        name: me.name ?? null,
        kind: me.kind,
        has: (permission) => rights.has(permission),
      },
    });
  }
  return state.value;
}

/** The session of the shell, reactive. */
export function useSession(): Readonly<ShallowRef<Session>> {
  return state;
}
