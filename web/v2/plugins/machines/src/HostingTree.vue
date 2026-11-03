<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The view « Hébergement et clusters » (`/x/machines/clusters`; task 18cd; design 021
// « Catalogue », plugin Machines, and « Déplacements dans les arbres »; REQ-021-32, 33): who
// hosts what, in a `TreeGrid` whose parent is declared. At the first level the clusters,
// then the machines without a host (a page of `/v1/machines` read until one shows, at most
// `MAX_PAGES`); under a cluster or a machine that may host (physical, virtual: M5), its
// guests (`/v1/machines?host=`), read once the node is opened, a few lists at a time
// (`limited`), never read again by themselves after a refusal (« Réessayer »). A machine moves
// by a drag, by Ctrl+X then Ctrl+V, or by « Déplacer vers… », each target judged by
// `canMove` of the SDK (a cycle, a type the contract does not admit, a right, the present),
// to the active draft by `draft.move`, which simulates it. Never optimistic (X33): no
// `setQueryData`, the node stays at its place, « en attente » until the verdict, then
// « en brouillon » there once its simulation holds it, or refused with the code of the rule;
// it changes place only when the lists read again, once the draft is applied, show it there.
// What a move says, its warnings among it (`machines.host.site_mismatch`), goes to a status
// region there from the start. Moves are offered at the present only (W9).
import {
  ApiProblem,
  canMove,
  draft,
  MAX_PAGES,
  reads,
  type DraftMove,
  type MoveVerdict,
  type Operation,
} from "@kartenn/sdk";
import {
  Button,
  Cluster as Inline,
  EmptyState,
  ErrorState,
  InlineAlert,
  Stack,
  TreeGrid,
  type GridColumn,
  type MoveCheck,
  type MoveIntent,
  type MoveState,
  type TreeNode,
  type TreeNodeState,
} from "@kartenn/ui";
import {
  computed,
  onScopeDispose,
  ref,
  shallowReactive,
  shallowRef,
  useTemplateRef,
  watch,
} from "vue";
import { useList, useLists, useNotified } from "./data.ts";
import { arrival, followLink, hrefOf, PAGES } from "./dom.ts";
import { useHosted, useTexts } from "./host.ts";
import { linesOf, type AlertLine } from "./problems.ts";
import { limited, Names, type Cluster, type Machine } from "./reads.ts";
import RetryAlert from "./RetryAlert.vue";

const MACHINE = "machines.machine";
const CLUSTER = "machines.cluster";
/** What a move asks of the user (`canMove` asks it again with the target's). */
const PERMISSION = "machines.edit";
/** Longest chain of hosts walked for the ancestors (M5: two levels; more is a loop). */
const MAX_DEPTH = 8;
/** The kinds that may host a machine (M5): a physical one, a virtual one for a container. */
const HOSTS = new Set(["physical", "virtual"]);

const t = useTexts();
const { context, element } = useHosted();
const names = useNotified((changed, live) => new Names(changed, live));
const tree = useTemplateRef<{ openMove(id: string): boolean }>("tree");
const present = computed(() => context.value.time.present);
const allowed = computed(() => context.value.user.has(PERMISSION));

// ---------------------------------------------------------------------------- the reads

const clusters = useList<Cluster, never>(() => reads.list("/v1/clusters") as never);
const machines = useList<Machine, never>(() => reads.list("/v1/machines") as never);

const expanded = ref<string[]>([]);
const selected = ref<string | undefined>();

// The guests of each node opened, a few lists at a time (`limited`, shared with the names):
// the list of a node is enabled once it has its place, which it holds until its first
// answer, the node open or closed meanwhile (its list followed until then). A node closed
// before its turn is not admitted: its place goes to the next one, it is asked again if
// opened again.
const admitted = shallowRef<ReadonlySet<string>>(new Set());
/** The nodes holding a place: their list awaited. */
const holding = shallowRef<ReadonlySet<string>>(new Set());
let asked = new Set<string>();
const places = new Map<string, () => void>();
let alive = true;
let round = 0;
function admit(ids: readonly string[]): void {
  const mine = round;
  for (const id of ids) {
    if (asked.has(id)) continue;
    asked.add(id);
    void limited(
      () => alive && mine === round && expanded.value.includes(id),
      () =>
        new Promise<void>((resolve) => {
          places.set(id, resolve);
          holding.value = new Set(places.keys());
          admitted.value = new Set(admitted.value).add(id);
        }),
    ).then((read) => {
      // Closed before its turn: not read, asked again once opened again.
      if (read === null && mine === round && !admitted.value.has(id)) asked.delete(id);
    });
  }
}
/** The place of `id` given back: its list answered, or the view gone. */
function release(id: string): void {
  places.get(id)?.();
  places.delete(id);
  holding.value = new Set(places.keys());
}
function giveBack(): void {
  for (const id of [...places.keys()]) release(id);
}
onScopeDispose(() => {
  alive = false;
  giveBack();
});
watch(expanded, admit, { immediate: true });

/** The lists followed: of the nodes opened, and of those still holding a place. */
const followed = computed(() => [
  ...expanded.value,
  ...[...holding.value].filter((id) => !expanded.value.includes(id)),
]);
const guests = useLists<Machine, never>(
  () =>
    followed.value.map((id) => ({
      ...(reads.list("/v1/machines", { host: id }) as object),
      enabled: admitted.value.has(id),
    })) as never,
);
// A list answered (read or refused), its node open or not: its place goes to the next one.
watch(guests.states, (states) => {
  followed.value.forEach((id, i) => {
    if (places.has(id) && admitted.value.has(id) && states[i] && !states[i].pending) release(id);
  });
});
const indexOf = (id: string) => followed.value.indexOf(id);
const guestsOf = (id: string) => {
  const i = indexOf(id);
  return i < 0 || !expanded.value.includes(id) ? undefined : guests.states.value[i];
};

// ----------------------------------------------------------------------------- the tree

const columns = computed<GridColumn[]>(() => [
  { id: "kind", label: t("machines.kind"), width: "sm" },
  { id: "site", label: t("machines.site"), width: "md", priority: 2 },
]);

/** A site in a cell: its slug once read, its identifier if unreadable, « aucun » if none. */
function siteOf(id: string | undefined): string {
  if (!id) return t("none");
  const name = names().of("core.site", id);
  return name.state === "read" ? name.name : name.state === "failed" ? id : t("loading");
}

/** The model: the nodes, the parent and the type of each, the host of each machine read. */
const model = computed(() => {
  const parents = new Map<string, string | null>();
  const types = new Map<string, string>();
  /** The kind of each machine (M5: who may host), the label of each node. */
  const kinds = new Map<string, string>();
  const labels = new Map<string, string>();
  const up = new Map<string, string>();
  for (const m of machines.state.value.items ?? []) {
    if (m.host) up.set(m.id, "machine" in m.host ? m.host.machine.id : m.host.cluster.id);
  }
  /** The guests of a host opened, once read; a leaf when it has none. */
  function withGuests(node: TreeNode): TreeNode {
    const list = guestsOf(node.id);
    if (!list?.items) return { ...node, hasChildren: true };
    const children = list.items.flatMap((m) => machineNode(m, node.id));
    const more = list.more && list.pages < MAX_PAGES && !list.error;
    return { ...node, children, more, hasChildren: children.length > 0 || more };
  }
  function machineNode(m: Machine, parent: string | null): TreeNode[] {
    // Once only: a list read again before another may hold a machine moved meanwhile.
    if (types.has(m.id)) return [];
    types.set(m.id, MACHINE);
    kinds.set(m.id, m.kind);
    labels.set(m.id, m.name);
    parents.set(m.id, parent);
    if (parent) up.set(m.id, parent);
    const node: TreeNode = {
      id: m.id,
      label: m.name,
      icon: "server",
      cells: { kind: t(`kind.${m.kind}`), site: siteOf(m.site?.id) },
    };
    return [HOSTS.has(m.kind) ? withGuests(node) : node];
  }
  const nodes: TreeNode[] = [];
  for (const c of clusters.state.value.items ?? []) {
    if (types.has(c.id)) continue;
    types.set(c.id, CLUSTER);
    labels.set(c.id, c.slug);
    parents.set(c.id, null);
    const node: TreeNode = {
      id: c.id,
      label: c.slug,
      icon: "cluster",
      cells: { kind: t("hosting.cluster"), site: siteOf(c.site?.id) },
    };
    nodes.push(withGuests(node));
  }
  for (const m of machines.state.value.items ?? []) {
    if (!m.host) nodes.push(...machineNode(m, null));
  }
  return { nodes, parents, types, kinds, labels, up, roots: nodes.length };
});

/** Pages of the first level left to read, within `MAX_PAGES`. */
const pagesLeft = (s: { more: boolean; pages: number; error: unknown }) =>
  s.more && s.pages < MAX_PAGES && !s.error;
const more = computed(() => pagesLeft(clusters.state.value) || pagesLeft(machines.state.value));
const moreLoading = computed(() => clusters.state.value.fetching || machines.state.value.fetching);
const partial = computed(() =>
  [clusters.state.value, machines.state.value, ...guests.states.value].some(
    (s) => s.more && s.pages >= MAX_PAGES,
  ),
);

/** The first level the tree asked a page for: the machines read until a new root shows. */
const wanted = ref<number | null>(null);
watch(
  () => [machines.state.value, wanted.value, model.value.roots] as const,
  ([s, want, roots]) => {
    if (want === null) return;
    if (roots > want || !pagesLeft(s)) wanted.value = null;
    else if (!s.fetching) machines.next();
  },
);

/** The tree asks the next page of a list: of the first level (`null`), or of a node. */
function onLoad(parent: string | null): void {
  if (parent !== null) {
    const i = indexOf(parent);
    if (i >= 0) guests.next(i);
    return;
  }
  const c = clusters.state.value;
  if (pagesLeft(c)) {
    if (!c.fetching) clusters.next();
    return;
  }
  wanted.value = model.value.roots;
}

/** The lists that failed: the first level before any of its nodes, or else any. */
const firstFailure = computed(() => {
  const c = clusters.state.value;
  const m = machines.state.value;
  return c.items === undefined && m.items === undefined ? (c.error ?? m.error ?? null) : null;
});
const laterFailure = computed(
  () => clusters.state.value.error ?? machines.state.value.error ?? null,
);
function retry(): void {
  if (clusters.state.value.error) clusters.retry();
  if (machines.state.value.error) machines.retry();
}
/** The guests of an opened node that could not be read, each with its host and its list. */
const guestFailures = computed(() =>
  expanded.value.flatMap((id) => {
    const i = indexOf(id);
    const error = guests.states.value[i]?.error;
    return error ? [{ id, index: i, error, host: model.value.labels.get(id) ?? id }] : [];
  }),
);
const loading = computed(
  () =>
    !firstFailure.value &&
    (clusters.state.value.items === undefined || machines.state.value.items === undefined),
);
const empty = computed(
  () => !loading.value && !firstFailure.value && model.value.nodes.length === 0 && !more.value,
);
const problemOf = (error: unknown) => (error instanceof ApiProblem ? error : undefined);

/** Enter on a row: the record of its cluster or its machine, its heading focused. */
function onActivate(id: string): void {
  const type = model.value.types.get(id);
  if (!type) return;
  arrival.byLink = true;
  followLink(element, hrefOf(type === CLUSTER ? PAGES.cluster(id) : PAGES.machine(id)));
}

// ----------------------------------------------------------------------------- the moves

/** What `canMove` knows: the hosts read, the rights, the present. */
const env = computed(() => {
  const { up } = model.value;
  const user = context.value.user;
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
    present: context.value.time.present,
  };
});

/** The reason of a refusal of `canMove`, in words, with the code of the server's rule. */
const refusalOf = (verdict: Extract<MoveVerdict, { ok: false }>): MoveCheck => ({
  ok: false,
  reason: t(`hosting.${verdict.refusal}`, { permission: verdict.permission ?? "" }),
  ...(verdict.code ? { code: verdict.code } : {}),
});

/** Whether `node` may go under `target` (`null`: no host), for each target hovered or listed. */
function check(node: TreeNode, target: TreeNode | null): MoveCheck {
  const { types, kinds, parents } = model.value;
  const type = types.get(node.id);
  if (type === CLUSTER) return { ok: false, reason: t("hosting.fixed") };
  const to = target ? types.get(target.id) : null;
  if (!type || to === undefined) return { ok: false, reason: t("hosting.incompatible") };
  if ((parents.get(node.id) ?? null) === (target?.id ?? null)) {
    return { ok: false, reason: t("hosting.current") };
  }
  // The kind of the target too: a container or an appliance hosts nothing (M5).
  const kind = target ? kinds.get(target.id) : undefined;
  const verdict = canMove(
    { id: node.id, type },
    target && to ? { id: target.id, type: to, ...(kind ? { kind } : {}) } : null,
    env.value,
  );
  return verdict.ok ? { ok: true } : refusalOf(verdict);
}

/** The moves in flight or refused, by node; the nodes moved in the draft, with their parent then. */
const states = shallowReactive(new Map<string, MoveState>());
const drafted = shallowReactive(new Map<string, string | null>());
/** What the last move says, in a status region there from the start; its warnings in it. */
const said = ref("");
const warnings = shallowRef<AlertLine[]>([]);
/** The simulation of the draft could not be done: an alert. */
const unsimulated = shallowRef<AlertLine[] | null>(null);

// Other dates: every list of guests read again, a few at a time; no move is offered there,
// what the last one said no longer holds.
watch(
  () => `${context.value.time.validAt ?? ""} ${context.value.time.asOf ?? ""}`,
  () => {
    round++;
    giveBack();
    asked = new Set();
    admitted.value = new Set();
    admit(expanded.value);
    said.value = "";
    warnings.value = [];
    unsimulated.value = null;
  },
);
// Another active draft, or none (left, deleted, submitted): the marks « en brouillon » no
// longer hold.
onScopeDispose(draft.onActive(() => drafted.clear()));

const moveState = (node: TreeNode): MoveState | undefined => states.get(node.id);
function nodeState(node: TreeNode): TreeNodeState | undefined {
  // At its place, as long as its reads keep it there (the draft not applied yet).
  if (drafted.has(node.id) && drafted.get(node.id) === (model.value.parents.get(node.id) ?? null)) {
    return "draft";
  }
  // Opened, its list awaited: not admitted yet, or read.
  if (!expanded.value.includes(node.id) || node.children) return undefined;
  return !admitted.value.has(node.id) || guestsOf(node.id)?.pending
    ? "loading-children"
    : undefined;
}

/** A refusal of the API, as the node shows it: its first rule, else its problem. */
function refusedBy(problem: unknown): MoveCheck {
  if (!(problem instanceof ApiProblem)) return { ok: false, reason: t("network") };
  const first = problem.violations[0];
  if (first) return { ok: false, reason: first.detail, code: first.rule };
  return { ok: false, reason: problem.detail ?? problem.title, code: problem.code };
}

/** A move asked by the tree, checked again: to the draft, then its simulation. */
async function onMove({ node, target }: MoveIntent): Promise<void> {
  // A second intent while one is in flight: ignored.
  if (states.get(node.id) === "pending") return;
  // What the last move said no longer holds.
  said.value = "";
  warnings.value = [];
  unsimulated.value = null;
  const verdict = check(node, target);
  if (!verdict.ok) {
    states.set(node.id, { refused: verdict });
    return;
  }
  const from = model.value.parents.get(node.id) ?? null;
  const to = target ? model.value.types.get(target.id) : null;
  const words = { machine: node.label, host: target?.label ?? t("hosting.root") };
  // The wait is the row's (« en attente »), after the tree's « demandé »: not said again.
  states.set(node.id, "pending");
  drafted.delete(node.id);
  const op = {
    op: "machines.update_machine",
    id: { id: node.id },
    host: target
      ? to === CLUSTER
        ? { cluster: { id: target.id } }
        : { machine: { id: target.id } }
      : null,
  } as Operation;
  let result: DraftMove;
  try {
    result = await draft.move(op, { id: node.id, type: MACHINE });
  } catch (failure) {
    said.value = t("move.failed", words);
    states.set(node.id, { refused: refusedBy(failure) });
    return;
  }
  if (result.kind !== "drafted") {
    said.value = t("move.failed", words);
    states.set(node.id, { refused: refusedBy(result.problem) });
    return;
  }
  if (result.verdict === "refused") {
    // Refused by a rule of the server: the node says it with its code, the operation stays
    // in the draft until another move of the node replaces it.
    const v = result.violation;
    states.set(node.id, { refused: { ok: false, reason: v.detail, code: v.rule } });
    said.value = t("hosting.kept", words);
    return;
  }
  states.delete(node.id);
  drafted.set(node.id, from);
  if (result.verdict === "unknown") {
    said.value = t("hosting.drafted", words);
    unsimulated.value = linesOf(result.problem, t);
    return;
  }
  warnings.value = result.warnings.map((w) => ({ message: w.detail, code: w.rule }));
  said.value = t("hosting.valid", words);
}

/** Why « Déplacer vers… » cannot start on the chosen node, if it cannot. */
const moveBlocked = computed(() => {
  const id = selected.value;
  if (!id || !model.value.types.has(id)) return t("hosting.pickFirst");
  if (model.value.types.get(id) === CLUSTER) return t("hosting.fixedCluster");
  if (states.get(id) === "pending") return t("hosting.inFlight");
  return undefined;
});
function moveTo(): void {
  if (selected.value && !moveBlocked.value) tree.value?.openMove(selected.value);
}
</script>

<template>
  <Stack gap="6">
    <Stack gap="1">
      <h1 tabindex="-1">{{ t("hosting.title") }}</h1>
      <p>{{ t("hosting.intro") }}</p>
      <!-- There from the start, whatever the state of the view, so that what it holds is
           said: the outcome of a move, its warnings with their codes. -->
      <div role="status">
        <Stack v-if="said || warnings.length" gap="1">
          <p v-if="said">{{ said }}</p>
          <Stack v-if="warnings.length" as="ul" gap="1">
            <li v-for="(w, i) in warnings" :key="i">
              {{ t("warning") }} {{ w.message }} <span class="k-mono">{{ w.code }}</span>
            </li>
          </Stack>
        </Stack>
      </div>
    </Stack>

    <ErrorState
      v-if="firstFailure"
      :title="t('hosting.failed')"
      :problem="problemOf(firstFailure)"
      :retry-label="t('retry')"
      @retry="retry"
    >
      {{ problemOf(firstFailure) ? "" : t("network") }}
    </ErrorState>
    <EmptyState v-else-if="empty" :title="t('hosting.none')">
      {{ t("hosting.noneText") }}
    </EmptyState>
    <Stack v-else gap="4">
      <RetryAlert
        v-if="laterFailure"
        :title="t('hosting.failed')"
        :error="laterFailure"
        @retry="retry"
      />
      <!-- Each list of guests refused, named by its host, read again by itself only. -->
      <RetryAlert
        v-for="f in guestFailures"
        :key="f.id"
        :title="t('hosting.guestsFailed', { host: f.host })"
        :error="f.error"
        @retry="guests.retry(f.index)"
      />
      <Inline v-if="present" gap="2">
        <Button
          icon="move"
          :forbidden="allowed ? undefined : PERMISSION"
          :reason="allowed ? moveBlocked : undefined"
          @click="moveTo"
          >{{ t("hosting.moveTo") }}</Button
        >
      </Inline>
      <p v-else>{{ t("hosting.datedView") }}</p>
      <InlineAlert
        v-if="unsimulated"
        tone="ko"
        :title="t('move.unsimulated')"
        :items="unsimulated"
      />
      <TreeGrid
        ref="tree"
        v-model:expanded="expanded"
        v-model:selected="selected"
        :label="t('hosting.tree')"
        :tree-label="t('hosting.name')"
        :nodes="model.nodes"
        :columns="columns"
        :more="more"
        :more-loading="moreLoading"
        :loading="loading"
        :loading-rows="4"
        :parent-mode="present ? 'declared' : 'derived'"
        :can-move="check"
        :move-state="moveState"
        :node-state="nodeState"
        @load="onLoad"
        @move="onMove"
        @activate="onActivate"
      />
      <p v-if="partial">{{ t("hosting.partial") }}</p>
    </Stack>
  </Stack>
</template>
