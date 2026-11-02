<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The section « Hébergement » of the record of a machine (task 18b, in Vue by 18cb;
// REQ-021-32, 33; design « Déplacements dans les arbres »): its new host chosen in a
// `Combobox` of the hosts read — none, the clusters, the machines, every page the cache holds
// (`MAX_PAGES`), so that the ancestors of each are known. Each host is judged by `canMove` of
// the SDK (a cycle, a type the contract does not admit, a right, the present); refused ones
// stay offered, their reason and the code of the rule of the server beside their name, and
// are refused again on the field if sent. The move goes to the active draft by `draft.move`,
// which simulates it: never optimistic, the host shown stays the one read, « en attente »
// until the answer; the server stays the only judge (its rules on the kinds of the pair, the
// site of the host). Offered at the present only, by the record (W9).
import {
  canMove,
  draft,
  MAX_PAGES,
  reads,
  type DraftMove,
  type MoveSubject,
  type MoveVerdict,
  type Operation,
} from "@kartenn/sdk";
import {
  Button,
  Cluster as Inline,
  Combobox,
  filterOptions,
  InlineAlert,
  Stack,
  type ListOption,
  type Suggest,
} from "@kartenn/ui";
import { computed, nextTick, ref, shallowRef, useTemplateRef, watch } from "vue";
import { useList, type List } from "./data.ts";
import { useHosted, usePresent, useTexts } from "./host.ts";
import { linesOf, type AlertLine } from "./problems.ts";
import type { Cluster, Machine, MachineCard } from "./reads.ts";
import RetryAlert from "./RetryAlert.vue";

const props = defineProps<{ id: string; machine: MachineCard | undefined }>();

/** What the move asks of the user (`canMove` asks it again with the target's). */
const PERMISSION = "machines.edit";
/** Longest chain of hosts walked for the ancestors (M5: two levels; more is a loop). */
const MAX_DEPTH = 8;
const MACHINE = "machines.machine";
const CLUSTER = "machines.cluster";

const t = useTexts();
const { context } = useHosted();
const present = usePresent();
const form = useTemplateRef<HTMLFormElement>("form");

const clusters = useList<Cluster, never>(() => reads.list("/v1/clusters") as never);
const machines = useList<Machine, never>(() => reads.list("/v1/machines") as never);
const lists = [clusters, machines] as {
  state: { value: List<unknown> };
  next(): void;
  retry(): void;
}[];
// Every page of them, as many as the cache holds: every host offered, its ancestors known;
// a page refused stops the reading (said, with « Réessayer »), never asked again by itself.
for (const list of lists) {
  watch(
    () => list.state.value,
    (s) => {
      if (s.more && !s.fetching && !s.previous && !s.error && s.pages < MAX_PAGES) list.next();
    },
    { immediate: true },
  );
}
/** The lists of hosts that failed, read again (the next pages follow by themselves). */
function retryLists(): void {
  for (const list of lists) if (list.state.value.error) list.retry();
}

const allowed = computed(() => context.value.user.has(PERMISSION));

/** A host offered: its value in the list, the node `canMove` judges, its name. */
interface Host {
  value: string;
  subject: MoveSubject | null;
  label: string;
  /** Its type, said beside its name. */
  type?: string;
}

/** The host of a machine as a value of the list: `machine:<id>`, `cluster:<id>`, `none`. */
function valueOf(host: Machine["host"]): string {
  if (!host) return "none";
  return "machine" in host ? `machine:${host.machine.id}` : `cluster:${host.cluster.id}`;
}

/** The hosts offered: none, the clusters by slug, the machines by name. */
const hosts = computed<Host[]>(() => [
  { value: "none", subject: null, label: t("move.none") },
  ...[...(clusters.state.value.items ?? [])]
    .sort((a, b) => a.slug.localeCompare(b.slug))
    .map((c) => ({
      value: `cluster:${c.id}`,
      subject: { id: c.id, type: CLUSTER },
      label: c.slug,
      type: t("move.cluster"),
    })),
  ...[...(machines.state.value.items ?? [])]
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((m) => ({
      value: `machine:${m.id}`,
      subject: { id: m.id, type: MACHINE },
      label: m.name,
      type: t("move.machine"),
    })),
]);

/** What `canMove` knows: the host machine of each machine read, the rights, the present. */
const env = computed(() => {
  const up = new Map<string, string>();
  for (const m of machines.state.value.items ?? []) {
    if (m.host && "machine" in m.host) up.set(m.id, m.host.machine.id);
  }
  const user = context.value.user;
  const now = context.value.time.present;
  return {
    ancestors: (id: string) => {
      const found: string[] = [];
      for (let at = up.get(id); at && found.length < MAX_DEPTH; at = up.get(at)) {
        if (found.includes(at)) break;
        found.push(at);
      }
      return found;
    },
    can: (permission: string) => user.has(permission),
    present: now,
  };
});

/** The reason of a refusal, in words, with the code of the rule of the server. */
function refusal(verdict: Extract<MoveVerdict, { ok: false }>): string {
  const reason = t(`move.${verdict.refusal}`, { permission: verdict.permission ?? "" });
  return verdict.code ? `${reason} (${verdict.code})` : reason;
}

/** Why a host is not offered to the move: its host now, or the first refusal of `canMove`. */
function refusedOf(host: Host): string | null {
  if (props.machine && valueOf(props.machine.host) === host.value) return t("move.current");
  const verdict = canMove({ id: props.id, type: MACHINE }, host.subject, env.value);
  return verdict.ok ? null : refusal(verdict);
}

/**
 * The options of the field: each host by its name, refused ones with their reason in it
 * (read with the option, kept in the field once taken), its type beside.
 */
const options = computed(() =>
  hosts.value.map((host): ListOption => {
    const refused = refusedOf(host);
    return {
      value: host.value,
      label: refused ? `${host.label} — ${refused}` : host.label,
      detail: host.type,
    };
  }),
);
const suggest: Suggest = (text) => Promise.resolve(filterOptions(options.value, text));

const search = ref("");
const chosen = ref<string | undefined>();
const error = ref<string | undefined>();
const moving = ref(false);
/** What the last move says, in a live region there from the start. */
const said = ref("");
/** Its refusal, or its failure: an alert. */
const alert = shallowRef<{ title: string; items: AlertLine[]; kept?: boolean } | null>(null);
const warnings = shallowRef<AlertLine[]>([]);

// What the last move said no longer holds once another host is chosen (not while one is
// sent: its wait stays said).
watch(chosen, () => {
  error.value = undefined;
  if (moving.value) return;
  said.value = "";
  alert.value = null;
  warnings.value = [];
});

/** Whether the hosts are read, failed, or still coming page after page. */
const listFailure = computed(
  () => clusters.state.value.error ?? machines.state.value.error ?? null,
);
const reading = computed(() =>
  [clusters.state.value, machines.state.value].some(
    (s) => !s.error && (s.items === undefined || s.fetching || (s.more && s.pages < MAX_PAGES)),
  ),
);

async function refuse(reason: string): Promise<void> {
  error.value = reason;
  await nextTick();
  form.value?.querySelector<HTMLElement>("[role='combobox']")?.focus();
}

/** The move of the host chosen, checked again: to the draft, then its simulation. */
async function submit(): Promise<void> {
  // A second intent while one is in flight: ignored.
  if (!present() || moving.value) return;
  if (!allowed.value) {
    // Emptied first: the same sentence twice is said twice.
    said.value = "";
    await nextTick();
    said.value = `${t("form.needs")} ${PERMISSION}`;
    return;
  }
  said.value = "";
  alert.value = null;
  warnings.value = [];
  const host = hosts.value.find((h) => h.value === chosen.value);
  if (!host) return refuse(search.value.trim() ? t("move.pick") : t("form.required"));
  // Its host now, or a host `canMove` refuses: said on the field, nothing sent.
  if (props.machine && valueOf(props.machine.host) === host.value) {
    return refuse(t("move.already"));
  }
  const verdict = canMove({ id: props.id, type: MACHINE }, host.subject, env.value);
  if (!verdict.ok) return refuse(refusal(verdict));
  moving.value = true;
  // The host shown stays the one read until its read shows another (X33).
  said.value = t("move.pending", { host: host.label });
  const subject = host.subject;
  const op = {
    op: "machines.update_machine",
    id: { id: props.id },
    host: subject
      ? subject.type === CLUSTER
        ? { cluster: { id: subject.id } }
        : { machine: { id: subject.id } }
      : null,
  } as Operation;
  try {
    outcome(await draft.move(op, { id: props.id, type: MACHINE }), host.label);
  } catch (failure) {
    said.value = "";
    alert.value = { title: t("move.failed", { host: host.label }), items: linesOf(failure, t) };
  } finally {
    moving.value = false;
  }
}

/** What the draft and its simulation said of the move. */
function outcome(result: DraftMove, host: string): void {
  if (result.kind !== "drafted") {
    said.value = "";
    alert.value = { title: t("move.failed", { host }), items: linesOf(result.problem, t) };
    return;
  }
  if (result.verdict === "refused") {
    // Refused by a rule of the server: the operation stays in the draft, a new choice
    // replaces it.
    said.value = "";
    const v = result.violation;
    alert.value = {
      title: t("move.refused", { host }),
      items: [{ message: v.detail, code: v.rule }],
      kept: true,
    };
    return;
  }
  if (result.verdict === "unknown") {
    said.value = t("move.drafted", { host });
    alert.value = { title: t("move.unsimulated"), items: linesOf(result.problem, t) };
    return;
  }
  said.value = t("move.valid", { host });
  warnings.value = result.warnings.map((w) => ({ message: w.detail, code: w.rule }));
}
</script>

<template>
  <form ref="form" novalidate @submit.prevent="submit">
    <Stack gap="4">
      <RetryAlert
        v-if="listFailure"
        :title="t('move.unreadable')"
        :error="listFailure"
        @retry="retryLists"
      />
      <Combobox
        v-model="search"
        v-model:value="chosen"
        :label="t('move.host')"
        :suggest="suggest"
        :hint="reading ? t('move.reading') : t('move.hint')"
        :error="error"
        required
      />
      <InlineAlert v-if="alert" tone="ko" :title="alert.title" :items="alert.items" />
      <p v-if="alert?.kept">{{ t("move.kept") }}</p>
      <InlineAlert
        v-if="warnings.length"
        tone="warn"
        :title="t('move.warnings')"
        :items="warnings"
      />
      <Inline gap="2">
        <Button
          variant="primary"
          type="submit"
          :loading="moving"
          :forbidden="allowed ? undefined : PERMISSION"
          >{{ t("move.submit") }}</Button
        >
        <!-- There from the start, so that what it holds is said. -->
        <span role="status">{{ said }}</span>
      </Inline>
    </Stack>
  </form>
</template>
