import { setContext } from "@kartenn/sdk/host";
import { shallowRef, type ShallowRef } from "vue";
import { NO_RIGHTS, Rights } from "./permissions.ts";

export { covers, Rights } from "./permissions.ts";

/** The caller as `GET /v1/me` sums them up (contract 006). */
export interface Me {
  identity: string;
  kind: string;
  name: string | null;
  groups: string[];
  roles: string[];
  bindings: { role: string; scope: { kind: string; id?: string } }[];
  global_permissions: string[];
}

export interface Session {
  me: Me | null;
  rights: Rights;
}

const state: ShallowRef<Session> = shallowRef({ me: null, rights: NO_RIGHTS });

/**
 * The entries of a caller: those of their global bindings, then those of the roles of
 * their scoped bindings, read by `GET /v1/roles/{key}` when the caller may (otherwise only
 * the server knows them: hidden here, never refused).
 */
export async function rightsOf(me: Me, fetcher: typeof fetch = fetch): Promise<Rights> {
  const scoped = new Set(me.bindings.filter((b) => b.scope.kind !== "global").map((b) => b.role));
  const entries = [...me.global_permissions];
  await Promise.all(
    [...scoped].map(async (key) => {
      const response = await fetcher(`/v1/roles/${encodeURIComponent(key)}`);
      if (response.ok) entries.push(...((await response.json()) as { entries: string[] }).entries);
    }),
  );
  return new Rights(entries);
}

/** Reads `/v1/me` again; signed out on 401. Pushes the user to the SDK's context. */
export async function loadSession(fetcher: typeof fetch = fetch): Promise<Session> {
  const response = await fetcher("/v1/me");
  const me = response.ok ? ((await response.json()) as Me) : null;
  const rights = me ? await rightsOf(me, fetcher) : NO_RIGHTS;
  state.value = { me, rights };
  if (me) {
    setContext({
      user: {
        identity: me.identity,
        name: me.name,
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
