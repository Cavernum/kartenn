import { api, keys, queryClient } from "@kartenn/sdk";
import { injectToasts } from "@kartenn/ui";
import { onScopeDispose, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import type { Refusal } from "../views/edit/inline.ts";
import { refusalOf, type SecretShown } from "./access.ts";

// The writes that make a secret of a service account (005, K8; REQ-007-19, REQ-021-12;
// maquette E7, vue `secret`): the creation of an account, with its first secret, and a new
// secret. The server alone makes a secret (the changeset holds its digest), so they are
// written at once (`?approval=self`, held for a decision otherwise), never through the
// draft. The secret answered is kept here, by the component that shows it, and nowhere
// else: neither in the cache of the reads (the answer is not a query; the lists are read
// again without it) nor in the draft; forgotten when its dialog closes or the component
// goes.

/** What the server answers a write that makes a secret (201 or 202). */
type Made = { state?: string; secret?: SecretShown | null };

export function useSecretWrite() {
  const { t } = useI18n();
  const toasts = injectToasts();
  /** The secret made, until it is shown and closed. */
  const secret = shallowRef<SecretShown | null>(null);
  /** Who it is for: the name of the account. */
  const account = ref("");
  const refusal = shallowRef<Refusal | null>(null);
  const busy = ref(false);
  const forget = () => {
    secret.value = null;
    account.value = "";
  };
  onScopeDispose(forget);

  async function run(name: string, write: () => Promise<{ data: unknown }>): Promise<boolean> {
    if (busy.value) return false;
    busy.value = true;
    refusal.value = null;
    try {
      const made = (await write()).data as Made | undefined;
      account.value = name;
      secret.value = made?.secret ?? null;
      if (made?.state && made.state !== "applied") {
        toasts?.show({ tone: "info", title: t("admin.secret.proposed") });
      }
      // The account and its secrets read again: the answer itself is never cached.
      await Promise.all(
        ["/v1/service-accounts", "/v1/service-accounts/{id}", "/v1/identities"].map((path) =>
          queryClient.invalidateQueries({ queryKey: keys.allOn(path) }),
        ),
      );
      return true;
    } catch (error) {
      refusal.value = refusalOf(error);
      return false;
    } finally {
      busy.value = false;
    }
  }

  /** Creates the account `name`, its first secret expiring at `expires` (none: never). */
  const create = (name: string, expires: string | null, motive: string) =>
    run(name, () =>
      api.post("/v1/service-accounts", { body: { name, expires_at: expires, motive } }),
    );

  /** Makes a new secret for the account `id`, named `name`. */
  const renew = (id: string, name: string, expires: string | null, motive: string) =>
    run(name, () =>
      api.post("/v1/service-accounts/{id}/secrets", {
        path: { id },
        body: { expires_at: expires, motive },
      }),
    );

  return { secret, account, refusal, busy, create, renew, forget };
}
