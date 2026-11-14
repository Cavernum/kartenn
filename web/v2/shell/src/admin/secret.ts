import { api, keys, type QueryClient } from "@kartenn/sdk";
import { injectToasts } from "@kartenn/ui";
import { onScopeDispose, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { useQueryClient } from "../data/query.ts";
import type { Refusal } from "../views/edit/inline.ts";
import { refusalOf, type SecretShown } from "./access.ts";
import { useAdminGate } from "./gate.ts";

// The writes that make a secret of a service account (005, K8; REQ-007-19, REQ-021-12;
// maquette E7, vue `secret`): the creation of an account, with its first secret, and a new
// secret. The server alone makes a secret (the changeset holds its digest), so they are
// written at once (`?approval=self`, held for a decision otherwise), never through the
// draft. The secret answered is kept here, by the component that shows it, and nowhere
// else: neither in the cache of the reads (the answer is not a query; the lists are read
// again without it) nor in the draft; forgotten when its dialog closes or the component
// goes. While the write is in flight its dialog does not close (`busy`); should the
// component go anyway (the route left), the answer that comes after is dropped, and a
// notice says a secret was made unseen, another to generate. Nothing is sent when the gate
// of writes refuses at the moment of sending (REQ-021-29: the past, under 1024 px wide).

/** What the server answers a write that makes a secret (201 or 202). */
type Made = { state?: string; secret?: SecretShown | null };

/** What the writes of a secret need: the API, the cache to read again, the words. */
export interface SecretDeps {
  post: (path: string, options: object) => Promise<{ data: unknown }>;
  queryClient: Pick<QueryClient, "invalidateQueries">;
  t: (key: string, values?: Record<string, unknown>) => string;
  notify: (tone: "info" | "warn", title: string) => void;
  /**
   * Why the gate of writes refuses now (`useAdminGate`: the past, a frame under 1024 px),
   * read at the moment of sending; `undefined` when it may be written.
   */
  held: () => string | undefined;
}

/** The writes of a secret, outside a component (tested alone); `dispose` when it goes. */
export function createSecretWrite(deps: SecretDeps) {
  /** The secret made, until it is shown and closed. */
  const secret = shallowRef<SecretShown | null>(null);
  /** Who it is for: the name of the account. */
  const account = ref("");
  const refusal = shallowRef<Refusal | null>(null);
  const busy = ref(false);
  let disposed = false;
  const forget = () => {
    secret.value = null;
    account.value = "";
  };
  const dispose = () => {
    disposed = true;
    forget();
  };

  async function run(name: string, write: () => Promise<{ data: unknown }>): Promise<boolean> {
    // The gate read when sending (the window may have narrowed since the dialog opened):
    // nothing sent, its dialog says why. A secret already shown stays (a read).
    if (busy.value || disposed || deps.held() !== undefined) return false;
    busy.value = true;
    refusal.value = null;
    try {
      const made = (await write()).data as Made | undefined;
      if (disposed) {
        // Nobody to show it to: dropped at once, and said.
        if (made?.secret) deps.notify("warn", deps.t("admin.secret.unseen", { account: name }));
      } else {
        account.value = name;
        secret.value = made?.secret ?? null;
        if (made?.state && made.state !== "applied") {
          deps.notify("info", deps.t("admin.secret.proposed"));
        }
      }
      // The account and its secrets read again: the answer itself is never cached.
      await Promise.all(
        ["/v1/service-accounts", "/v1/service-accounts/{id}", "/v1/identities"].map((path) =>
          deps.queryClient.invalidateQueries({ queryKey: keys.allOn(path) }),
        ),
      );
      return !disposed;
    } catch (error) {
      if (!disposed) refusal.value = refusalOf(error);
      return false;
    } finally {
      busy.value = false;
    }
  }

  /** Creates the account `name`, its first secret expiring at `expires` (none: never). */
  const create = (name: string, expires: string | null, motive: string) =>
    run(name, () =>
      deps.post("/v1/service-accounts", { body: { name, expires_at: expires, motive } }),
    );

  /** Makes a new secret for the account `id`, named `name`. */
  const renew = (id: string, name: string, expires: string | null, motive: string) =>
    run(name, () =>
      deps.post("/v1/service-accounts/{id}/secrets", {
        path: { id },
        body: { expires_at: expires, motive },
      }),
    );

  return { secret, account, refusal, busy, create, renew, forget, dispose };
}

export function useSecretWrite() {
  const { t } = useI18n();
  const toasts = injectToasts();
  const gate = useAdminGate();
  const write = createSecretWrite({
    post: (path, options) => api.post(path as "/v1/service-accounts", options as never),
    queryClient: useQueryClient(),
    t: (key, values) => (values ? t(key, values) : t(key)),
    notify: (tone, title) => toasts?.show({ tone, title }),
    held: () => gate.why.value,
  });
  onScopeDispose(write.dispose);
  return { ...write, gate };
}
