import { drafts, type Draft, type Entry, type Operation } from "@kartenn/sdk";
import { addToActiveDraft } from "@kartenn/sdk/host";
import { injectToasts } from "@kartenn/ui";
import { computed, onScopeDispose, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../data/query.ts";
import type { Refusal } from "../views/edit/inline.ts";

// The active draft as the administration reads and writes it (design 021 « Administration » :
// « chaque action va au brouillon »): its operations, which mark the rows of the lists; an
// action adds its operations at the end (a draft created when the tab has none), said by a
// toast; the action of a marked row takes its operation out (`PATCH`, under `If-Match`).
// A refusal of the server is kept for the panel to say it (`RefusalAlert`).

export function useAdminDraft() {
  const { t } = useI18n();
  const toasts = injectToasts();
  const active = ref<string | null>(drafts.active());
  onScopeDispose(drafts.onActive((id) => (active.value = id)));
  const query = useQuery(() => ({
    ...drafts.get(active.value ?? "-"),
    enabled: active.value !== null,
  }));
  const draft = computed(() =>
    active.value === null ? undefined : (query.data.value as Entry<Draft> | undefined)?.data,
  );
  const operations = computed<readonly Operation[]>(() => draft.value?.operations ?? []);
  const refusal = shallowRef<Refusal | null>(null);
  const busy = ref(false);

  /** Adds `ops` to the draft; `what` names them in the toast. Whether they went in. */
  async function add(ops: readonly Operation[], what: string): Promise<boolean> {
    if (busy.value) return false;
    busy.value = true;
    refusal.value = null;
    const outcome = await addToActiveDraft([...ops]);
    busy.value = false;
    if (outcome.kind !== "saved") {
      refusal.value = outcome as Refusal;
      return false;
    }
    toasts?.show({ tone: "ok", title: t("edit.drafted", { what }) });
    return true;
  }

  /** Takes the operation of index `index` out of the active draft. */
  async function drop(index: number, what: string): Promise<boolean> {
    const id = active.value;
    if (busy.value || id === null) return false;
    busy.value = true;
    refusal.value = null;
    const kept = operations.value.filter((_, i) => i !== index);
    const outcome = await drafts.update(id, { operations: [...kept] });
    busy.value = false;
    if (outcome.kind !== "saved") {
      refusal.value = outcome as Refusal;
      return false;
    }
    toasts?.show({ tone: "ok", title: t("admin.undrafted", { what }) });
    return true;
  }

  /**
   * Puts `op` in place of the operation of index `index` of the active draft. Unlike
   * `moveInDraft`, it never dates the draft: the administration writes at the present only
   * (its refusals say `edit.why.past` under any date, `valid_at` alone included).
   */
  async function replace(index: number, op: Operation, what: string): Promise<boolean> {
    const id = active.value;
    if (busy.value || id === null) return false;
    busy.value = true;
    refusal.value = null;
    const next = operations.value.map((o, i) => (i === index ? op : o));
    const outcome = await drafts.update(id, { operations: [...next] });
    busy.value = false;
    if (outcome.kind !== "saved") {
      refusal.value = outcome as Refusal;
      return false;
    }
    toasts?.show({ tone: "ok", title: t("edit.drafted", { what }) });
    return true;
  }

  /**
   * Defines the role `key` (`set_role`): the definition the draft already holds for it is
   * replaced, or taken out when the role comes back to what it is; otherwise one is added.
   */
  async function writeRole(
    role: { key: string; labels: Record<string, string>; entries: readonly string[] } | null,
    key: string,
    labels: Record<string, string>,
    entries: readonly string[],
    what: string,
  ): Promise<boolean> {
    const op = { op: "set_role", key, labels, entries: [...entries] } as Operation;
    const index = operations.value.findIndex(
      (o) =>
        (o as { op: string; key?: string }).op === "set_role" &&
        (o as { key?: string }).key === key,
    );
    if (index < 0) return add([op], what);
    const same =
      role !== null &&
      JSON.stringify(role.labels) === JSON.stringify(labels) &&
      role.entries.length === entries.length &&
      role.entries.every((e) => entries.includes(e));
    return same ? drop(index, what) : replace(index, op, what);
  }

  return { operations, refusal, busy, add, drop, replace, writeRole };
}
