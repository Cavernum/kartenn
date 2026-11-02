<script setup lang="ts">
// The edit panel of a kind (REQ-021-36, 37; design 021 « Panneau d'édition par sorte »;
// maquette E1, « + » sur un nœud): in the Détail tab of the right panel, the fields of the
// creation or of the merge patch of the kind (contract, `fields.ts`), the same wherever it
// is opened from — a list (`c`, `e`), a record (« Modifier »), a node of the topography
// (« + »: address, range or prefix, the domain given), the palette. The split button adds
// to the draft (the version read as `expected`) or applies at once (`?approval=self`, a
// motive prefilled): « Appliquer » without `X.approval.self` stays visible, refused, and
// names the permission. Ctrl+Enter runs the chosen action. Answers as in place: 202 held
// (never « appliqué »), 412 the input kept beside the values read again, 422 the
// violations on their fields with their codes, 403 a refusal. « Changer de parent… » of an
// entity or a site (REQ-021-32, 33) lists the valid targets only (`canMove`, `moves.ts`) and
// moves by the same split button: to the draft, or a changeset applied now; the record and
// the lists show the new parent once read again, never before. « Créer un VLAN pour ce
// réseau » (REQ-026-6, 021/A13): the creation and the setting of `core.vlan` on the network
// by a local name, two operations of one changeset, to the draft or applied at once; a
// refusal of the setting said on the field of the network.
import { addToActiveDraft } from "@kartenn/sdk/host";
import {
  changesets,
  keys,
  reads,
  write,
  type Entry,
  type Submission,
  type WriteOutcome,
} from "@kartenn/sdk";
import {
  Button,
  Checkbox,
  Cluster,
  CodeBadge,
  Combobox,
  filterOptions,
  InlineAlert,
  Select,
  Skeleton,
  SplitButton,
  Stack,
  TextField,
  injectToasts,
  type ListOption,
  type SplitAction,
} from "@kartenn/ui";
import { computed, nextTick, ref, useTemplateRef, watch, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery, useQuery } from "../../data/query.ts";
import { useSession } from "../../rights/index.ts";
import { useTime } from "../../time/index.ts";
import {
  collectionById,
  collectionOfKind,
  nameOf,
  type CollectionId,
  type Item,
} from "../collections.ts";
import ProblemState from "../../problems/ProblemState.vue";
import { problemCode, useProblems } from "../../problems/index.ts";
import { rowsOf, useKeyLabel } from "../data.ts";
import {
  ANCHOR_KINDS,
  applyRight,
  changesOf,
  checkValue,
  editRight,
  EDITS,
  fieldOfPointer,
  initialValues,
  patchOf,
  propertyRight,
  textOf,
  type EditField,
  type Values,
} from "./fields.ts";
import AttributeList from "../parts/AttributeList.vue";
import { VLAN_PROPERTY } from "../parts/vlan.ts";
import type { Refusal } from "./inline.ts";
import { useMoves, type MoveOutcome } from "./moves.ts";
import { closeEdit, type ShownRequest } from "./panel.ts";
import RefusalAlert from "./RefusalAlert.vue";
import { useSay } from "./say.ts";
import { draftLocal, nextFreeVid, ofAssignment, vlanForNetwork } from "./vlan.ts";

const props = defineProps<{ request: ShownRequest }>();
const { t, te } = useI18n();
const session = useSession();
const time = useTime();
const keyLabel = useKeyLabel();
// The panel closes once the write went through: « ajouté au brouillon » said by a toast too.
const toasts = injectToasts();
const say = useSay((text) => toasts?.show({ tone: "ok", title: text }));

const chosenKind = ref<CollectionId>(props.request.collection);
const collection = computed(() => collectionById(chosenKind.value)!);
const kindWords = computed(() => keyLabel("kind", collection.value.type) ?? collection.value.kind);
const creating = computed(() => props.request.mode === "create");
const moving = computed(() => props.request.mode === "move");
const objectId = computed(() => (props.request.mode === "create" ? null : props.request.id));
const fields = computed<readonly EditField[]>(() => {
  const edit = EDITS[collection.value.id];
  return creating.value ? edit.creation : moving.value ? (edit.move ?? []) : edit.modification;
});
/** The network a VLAN created is set on (« Créer un VLAN pour ce réseau »), if any. */
const network = computed(() =>
  props.request.mode === "create" && chosenKind.value === "vlans"
    ? props.request.network
    : undefined,
);

// The object modified, read at the present (the same entry as its record).
const record = useQuery(() => ({
  ...reads.object<Item>(collection.value.kind, objectId.value ?? "-"),
  enabled: objectId.value !== null,
}));
const item = computed(() => (record.data.value as Entry<Item> | undefined)?.data ?? null);
const name = computed(() => (item.value ? nameOf(item.value) : ""));

// The objects a reference may name: the domains, the parents of an entity or a site, the
// group of a VLAN; an anchor, an entity, a site or a domain (026 V3).
const targets = computed(
  () =>
    new Set(
      fields.value.flatMap((field): CollectionId[] =>
        field.kind === "anchor"
          ? ["entities", "sites", "domains"]
          : field.target
            ? [field.target]
            : [],
      ),
    ),
);
const targetRead = (id: CollectionId) =>
  useInfiniteQuery(() => ({
    ...reads.list(collectionById(id)!.path as "/v1/domains"),
    enabled: targets.value.has(id),
  }));
const READS: Partial<Record<CollectionId, ReturnType<typeof targetRead>>> = {
  domains: targetRead("domains"),
  entities: targetRead("entities"),
  sites: targetRead("sites"),
  "vlan-groups": targetRead("vlan-groups"),
};
const itemsOf = (id: CollectionId) => rowsOf(READS[id]?.data.value as never) as unknown as Item[];
/** The read of the nodes a move chooses among: the entities or the sites. */
const nodesRead = computed(() => READS[collection.value.id === "sites" ? "sites" : "entities"]!);
// A move reads the pages after the first (ten at most): every valid target offered, and the
// ancestors of each known for the check of a cycle.
const MOVE_PAGES = 10;
watchEffect(() => {
  const nodes = nodesRead.value;
  if (
    moving.value &&
    nodes.hasNextPage.value &&
    !nodes.isFetching.value &&
    !nodes.isFetchNextPageError.value &&
    (nodes.data.value?.pages.length ?? 0) < MOVE_PAGES
  ) {
    void nodes.fetchNextPage();
  }
});
/**
 * The option « Aucun » of an optional reference (a root): the empty string is the Select's
 * « no value », its placeholder, so the option has a value of its own, read back as `""`.
 */
const NONE = "none";
const selectValue = (field: EditField) => {
  const text = String(values.value[field.name] ?? "");
  return text === "" && field.kind === "reference" && !field.required ? NONE : text;
};
const fromSelect = (value: string | undefined) => (value === NONE ? "" : (value ?? ""));
function optionsOf(field: EditField): ListOption[] {
  if (field.kind === "choice") {
    return (field.options ?? []).map((key) => {
      const words = `views.value.${field.name}.${key}`;
      return { value: key, label: te(words) ? t(words) : key, detail: key };
    });
  }
  if (field.kind === "anchor") {
    return ANCHOR_KINDS.flatMap((kind) => {
      const of = collectionOfKind(kind)!;
      const words = keyLabel("kind", of.type) ?? kind;
      return itemsOf(of.id).map((i) => ({
        value: `${kind}:${String(i.id)}`,
        label: nameOf(i),
        detail: words,
        mono: true,
      }));
    });
  }
  const items = field.target ? itemsOf(field.target) : [];
  // A move lists the valid targets only (REQ-021-32): the others are not offered at all.
  const valid = (id: string | null, label: string) =>
    !moving.value || moves.canMove(movedNode.value, id === null ? null : { id, label }).ok;
  const options = items
    .filter((i) => valid(String(i.id), nameOf(i)))
    .map((i) => ({ value: String(i.id), label: nameOf(i), mono: true }));
  return field.required || !valid(null, t("edit.none"))
    ? options
    : [{ value: NONE, label: t("edit.none") }, ...options];
}

// « Changer de parent… »: a `Combobox` of the valid targets only (REQ-021-32), filtered by
// the text typed; the target taken, or « Aucun » for a root, is the field's value.
const moveText = ref("");
const moveValue = ref<string | undefined>(undefined);
const suggestTargets = async (query: string) => filterOptions(optionsOf(fields.value[0]!), query);
watch(moveValue, (value) => {
  if (moving.value && value !== undefined) setValue("parent", fromSelect(value));
});

// « Changer de parent… »: the node moved, its parent read, and the ancestors of each target
// as the pages read show them (a cycle beyond them is the server's to refuse).
const parentIds = computed(() => {
  const items = rowsOf(nodesRead.value.data.value as never) as unknown as Item[];
  return new Map(items.map((i) => [String(i.id), textOf(i.parent) || null]));
});
const movedNode = computed(() => ({ id: objectId.value ?? "", label: name.value }));
const moves = useMoves({
  subject: (node) => ({
    id: node.id,
    type: collection.value.type,
    ...(node.id === objectId.value
      ? { version: version.value, parent: textOf(item.value?.parent) || null }
      : {}),
  }),
  ancestors(id) {
    const up: string[] = [];
    for (let at = parentIds.value.get(id); at && !up.includes(at); at = parentIds.value.get(at)) {
      up.push(at);
    }
    return up;
  },
});

// The form: filled once, from the object read or the values given; the input is never
// replaced by a read that comes later (a 412 reads the object again).
const values = ref<Values>({});
const before = ref<Values>({});
const version = ref<string | null>(null);
const filled = ref(false);
function fill(): void {
  const start = initialValues(
    fields.value,
    item.value,
    props.request.mode === "create" ? (props.request.preset ?? {}) : {},
  );
  values.value = { ...start };
  before.value = { ...start };
  version.value = typeof item.value?.version === "string" ? item.value.version : null;
  if (moving.value) {
    // The parent read, by its slug (the summary of the record), or « Aucun ».
    const parent = item.value?.parent as { slug?: string | null } | null | undefined;
    moveValue.value = start.parent ? String(start.parent) : NONE;
    moveText.value = parent ? (parent.slug ?? String(start.parent)) : t("edit.none");
  }
  filled.value = true;
}
let filledFor = "";
watch(
  [() => props.request.serial, chosenKind, item],
  ([serial, kind, read]) => {
    if (filledFor !== `${serial}:${kind}`) filled.value = false;
    if (!filled.value && (creating.value || read)) {
      fill();
      filledFor = `${serial}:${kind}`;
    }
  },
  { immediate: true },
);
/** An input of the user: a new submission, under a new idempotency key. */
function setValue(name: string, value: string | boolean): void {
  values.value = { ...values.value, [name]: value };
  idempotencyKey = crypto.randomUUID();
  if (name === "vid") vidProposed.value = null;
}

// A VLAN created (026, REQ-008-20): the VID proposed is the next free one of the group
// chosen, until the user types one.
const vidProposed = ref<string | null>(null);
const { sentence } = useProblems();
watch(
  () => (creating.value && chosenKind.value === "vlans" ? String(values.value.group ?? "") : ""),
  async (group) => {
    if (group === "" || (values.value.vid !== "" && vidProposed.value === null)) return;
    let vid: number | null;
    try {
      vid = await nextFreeVid(group);
    } catch (error) {
      // No VID proposed: why, said by the status of the panel (`problems/`).
      if (values.value.group === group) {
        const code = problemCode(error);
        notice.value = t("edit.vlan.vidUnread", {
          why: code ? `${sentence(error)} (${code})` : sentence(error),
        });
      }
      return;
    }
    if (vid === null || values.value.group !== group) return;
    if (values.value.vid !== "" && vidProposed.value === null) return;
    values.value = { ...values.value, vid: String(vid) };
    vidProposed.value = group;
    // The value changed under the user: said by the status of the panel (4.1.3).
    notice.value = t("edit.vlan.vidSaid", { vid });
  },
  // A group given beforehand (« Créer un VLAN » of a group) proposes its VID at once.
  { immediate: true },
);
const hintOf = (field: EditField) =>
  field.name === "vid" && vidProposed.value !== null
    ? t("edit.vlan.vidProposed", {
        group: optionsOf(fields.value.find((f) => f.name === "group")!).find(
          (o) => o.value === vidProposed.value,
        )?.label,
      })
    : undefined;

// The action of the split button: to the draft by default; « Appliquer » with the right.
const action = ref("draft");
const canEdit = computed(() => session.value.rights.has(editRight(collection.value.type)));
const canApply = computed(
  () => canEdit.value && session.value.rights.has(applyRight(collection.value.type)),
);
/**
 * The permission an action lacks: `X.edit` for both, `X.approval.self` to apply; a VLAN set
 * on a network also writes the network and its property `core.vlan`.
 */
const lacking = (id: string) => {
  if (!canEdit.value) return editRight(collection.value.type);
  if (network.value) {
    const missing = [editRight("core.prefix"), propertyRight(VLAN_PROPERTY)].find(
      (p) => !session.value.rights.has(p),
    );
    if (missing) return missing;
  }
  return id === "apply" && !canApply.value ? applyRight(collection.value.type) : null;
};
const actions = computed<SplitAction[]>(() =>
  [
    { id: "draft", label: t("edit.toDraft"), icon: "draft" as const },
    { id: "apply", label: t("edit.apply"), icon: "circle-check" as const },
  ].map((a) => {
    const permission = lacking(a.id);
    return permission ? { ...a, forbidden: permission } : a;
  }),
);
const applying = computed(() => action.value === "apply" && canApply.value);
// The generic motive, prefilled until the user writes one of their own.
const motive = ref("");
let motiveTyped = false;
/** A move's destination, in its words: the target chosen, or the root. */
const destination = computed(() => {
  if (!moving.value) return "";
  const parent = String(values.value.parent ?? "");
  if (parent === "") return t("edit.move.root");
  return optionsOf(fields.value[0]!).find((o) => o.value === parent)?.label ?? parent;
});
watch(
  [name, kindWords, destination],
  () => {
    if (motiveTyped) return;
    const words = { kind: kindWords.value, name: name.value, to: destination.value };
    motive.value = network.value
      ? t("edit.vlan.motiveFor", { network: network.value.name })
      : creating.value
        ? t("edit.motiveCreate", words)
        : t(moving.value ? "edit.move.motive" : "edit.motiveModify", words);
  },
  { immediate: true },
);
function setMotive(text: string): void {
  motive.value = text;
  motiveTyped = true;
  idempotencyKey = crypto.randomUUID();
}

const title = computed(() => {
  const request = props.request;
  if (request.mode === "edit") return t("edit.modify", { kind: kindWords.value, name: name.value });
  if (request.mode === "move") {
    return t("edit.move.title", { kind: kindWords.value, name: name.value });
  }
  if (network.value) return t("edit.vlan.createFor", { network: network.value.name });
  return request.within
    ? t("edit.addIn", { within: request.within })
    : t("edit.create", { kind: kindWords.value });
});

// The answers: errors of the fields (local check, then 422), a refusal kept with the input.
const errors = ref<Record<string, string>>({});
const refusal = ref<Refusal | null>(null);
const notice = ref<string | null>(null);
const busy = ref(false);
/**
 * The key of a creation applied at once: kept while the user changes nothing, so that a try
 * again after a lost answer replays the same submission (REQ-006-15).
 */
let idempotencyKey = crypto.randomUUID();
/** The focus to the first field refused (local check, 422), its error read with it. */
async function toFirstError(): Promise<void> {
  await nextTick();
  root.value?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus();
}

const labelOf = (field: EditField) => t(`views.field.${field.name}`);
const shownText = (field: EditField, value: unknown) => {
  const text = textOf(value);
  if (field.kind === "flag") return t(`views.value.boolean.${value === true}`);
  if (field.kind === "reference" || field.kind === "anchor") {
    return optionsOf(field).find((o) => o.value === text)?.label ?? text;
  }
  return text || "—";
};
/** After a 412: each field whose value read again differs from the input. */
const changed = computed(() => {
  const current = refusal.value?.kind === "conflict" ? (refusal.value.current?.data as Item) : null;
  if (!current) return [];
  return fields.value
    .filter((field) => textOf(current[field.name]) !== textOf(values.value[field.name]))
    .map((field) => ({
      label: labelOf(field),
      current: shownText(field, current[field.name]),
      input: shownText(field, values.value[field.name]),
    }));
});

function checked(): boolean {
  const found: Record<string, string> = {};
  for (const field of fields.value) {
    const refused = checkValue(field, values.value[field.name] ?? "");
    if (refused) {
      found[field.name] = t(refused, {
        values: (field.options ?? []).join(", "),
        field: labelOf(field),
      });
    }
  }
  errors.value = found;
  return Object.keys(found).length === 0;
}

function settle(outcome: WriteOutcome<unknown> | { kind: "saved" }, what: string): void {
  busy.value = false;
  if (outcome.kind === "saved") {
    say.status(t("edit.drafted", { what }));
    closeEdit();
    return;
  }
  if (outcome.kind === "applied") {
    say.applied(what, outcome.changeset);
    closeEdit();
    return;
  }
  if (outcome.kind === "proposed") {
    say.proposed(what, outcome.changeset, outcome.reason);
    closeEdit();
    return;
  }
  const refused = outcome as Refusal;
  refusal.value = refused;
  if (refused.kind === "invalid") {
    // Each violation on the field its pointer names: a member of the patch, or of the one
    // operation of a creation (`/operations/0/prefix` → « Valeur »); for a VLAN created for
    // a network, the setting of `core.vlan` (`/operations/1/…`) on the field of the network.
    const names = new Set(fields.value.map((field) => field.name));
    if (network.value) names.add("network");
    const byField: Record<string, string> = {};
    for (const violation of [...Object.values(refused.byField).flat(), ...refused.general]) {
      const name =
        network.value && ofAssignment(violation)
          ? "network"
          : fieldOfPointer(collection.value.id, violation.pointer);
      if (!name || !names.has(name)) continue;
      const said = `${violation.detail} (${violation.rule})`;
      byField[name] = byField[name] ? `${byField[name]} ${said}` : said;
    }
    errors.value = byField;
    void toFirstError();
  }
  if (refused.kind === "conflict") {
    // Applying again writes on the version read again; the input stays.
    const current = refused.current?.data as Item | undefined;
    if (current) {
      before.value = initialValues(fields.value, current);
      version.value = typeof current.version === "string" ? current.version : version.value;
    }
  }
}

async function run(id: string = action.value): Promise<void> {
  if (busy.value || lacking(id) !== null) return;
  notice.value = null;
  refusal.value = null;
  // Checked again when it is written: nothing off the present, the draft included.
  if (!time.value.present) {
    notice.value = t("edit.why.past");
    return;
  }
  if (!checked()) {
    void toFirstError();
    return;
  }
  const edit = EDITS[collection.value.id];
  const what = network.value
    ? t("edit.vlan.whatFor", { vlan: nameOf(values.value as Item), network: network.value.name })
    : creating.value
      ? t("edit.whatCreated", { kind: kindWords.value, name: nameOf(values.value as Item) })
      : t("edit.whatModified", { kind: kindWords.value, name: name.value });
  if (id === "apply" && motive.value.trim() === "") {
    errors.value = { ...errors.value, motive: t("edit.motiveMissing") };
    void toFirstError();
    return;
  }
  if (moving.value) {
    await moveTo(String(values.value.parent ?? ""), id === "apply", what);
    return;
  }
  const changes = creating.value
    ? values.value
    : changesOf(fields.value, before.value, values.value);
  if (!creating.value && Object.keys(changes).length === 0) {
    notice.value = t("edit.unchanged");
    return;
  }
  busy.value = true;
  // A VLAN for a network: its creation and the setting of `core.vlan` by its local name, one
  // free in the draft (the names hold for the whole changeset).
  const created = async (draft: boolean) => {
    const op = edit.create(values.value);
    return network.value ? vlanForNetwork(op, network.value, draft ? await draftLocal() : 0) : [op];
  };
  if (id === "draft") {
    const ops = creating.value
      ? await created(true)
      : [edit.update(objectId.value!, version.value, changes)];
    const outcome = await addToActiveDraft(ops);
    settle(outcome as WriteOutcome<unknown> | { kind: "saved" }, what);
    return;
  }
  if (creating.value) {
    const submission: Submission = {
      motive: motive.value.trim(),
      operations: await created(false),
    };
    settle(await changesets.submit(submission, idempotencyKey, { approvalSelf: true }), what);
    return;
  }
  const outcome = await write
    .quick({
      key: keys.object(collection.value.kind, objectId.value!),
      patch: patchOf(fields.value, changes),
      motive: motive.value.trim(),
      ...(version.value ? { ifMatch: `"${version.value}"` } : {}),
    })
    .catch(() => ({ kind: "failed", input: {}, problem: null }) as Refusal);
  settle(outcome, what);
}

/**
 * « Changer de parent… »: the node under `parent` (`""`: a root), to the draft or applied now;
 * nothing is shown at its new place before the answer (X33). The simulation of the draft,
 * or the server, may still refuse it: the reason and its code on the field.
 */
async function moveTo(parent: string, now: boolean, what: string): Promise<void> {
  // A text typed and no target taken from the list: nothing is guessed.
  if (moveValue.value === undefined) {
    errors.value = { parent: t("edit.move.choose") };
    void toFirstError();
    return;
  }
  if (parent === (before.value.parent ?? "")) {
    notice.value = t("edit.unchanged");
    return;
  }
  busy.value = true;
  const label = optionsOf(fields.value[0]!).find((o) => o.value === parent)?.label ?? parent;
  const outcome: MoveOutcome = await moves.move(
    { node: movedNode.value, target: parent ? { id: parent, label } : null },
    { now, motive: motive.value.trim(), idempotencyKey },
  );
  const refused = (check: { reason: string; code?: string }) => {
    busy.value = false;
    errors.value = { parent: check.code ? `${check.reason} (${check.code})` : check.reason };
    void toFirstError();
  };
  if (outcome.kind === "unchanged") {
    busy.value = false;
    notice.value = t("edit.unchanged");
  } else if (outcome.kind === "refused") refused(outcome.check);
  else if (outcome.kind === "drafted") {
    const state = moves.moveState(movedNode.value);
    if (outcome.verdict === "unknown") {
      // In the draft, its simulation not done: the problem said, the panel kept.
      busy.value = false;
      notice.value = t("edit.move.unsimulated", { what });
      refusal.value = outcome.failure ?? null;
    } else if (outcome.verdict === "valid" || typeof state !== "object" || state.refused.ok) {
      settle({ kind: "saved" }, what);
    } else {
      // In the draft, but its simulation refuses it: its operation kept there, marked
      // refused; another target chosen here replaces it.
      notice.value = t("edit.move.keptRefused", { what });
      refused(state.refused);
    }
  } else settle(outcome, what);
}

function onKeydown(event: KeyboardEvent): void {
  if (event.defaultPrevented) return;
  if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    void run();
  }
}

/** The kind chosen among those offered (« + » of a node): its own fields, empty. */
function chooseKind(id: CollectionId): void {
  if (id === chosenKind.value) return;
  chosenKind.value = id;
  errors.value = {};
  refusal.value = null;
}

// The focus to the first field once the form shows (the panel opened by a key or a button).
const root = useTemplateRef<HTMLFormElement>("root");
watch(
  [() => props.request.serial, filled],
  async ([, ready]) => {
    if (!ready) return;
    await nextTick();
    root.value?.querySelector<HTMLElement>(".k-edit-panel__field :is(input, button)")?.focus();
  },
  { immediate: true },
);
</script>

<template>
  <ProblemState
    v-if="record.isError.value"
    :title="t('edit.readError', { kind: kindWords })"
    :error="record.error.value"
    :retry="record.refetch"
  />
  <Skeleton v-else-if="!filled" variant="rows" :rows="4" />
  <form
    v-else
    ref="root"
    class="k-edit-panel"
    :aria-label="title"
    novalidate
    @submit.prevent="run()"
    @keydown="onKeydown"
  >
    <Stack gap="3">
      <h2 class="k-edit-panel__title">{{ title }}</h2>
      <Cluster
        v-if="props.request.mode === 'create' && props.request.kinds"
        gap="1"
        role="group"
        :aria-label="t('edit.kind')"
      >
        <Button
          v-for="kind in props.request.kinds"
          :key="kind"
          size="sm"
          :icon="collectionById(kind)!.icon"
          :pressed="kind === chosenKind"
          @click="chooseKind(kind)"
          >{{ keyLabel("kind", collectionById(kind)!.type) ?? kind }}</Button
        >
      </Cluster>
      <template v-for="field in fields" :key="`${chosenKind}:${field.name}`">
        <Checkbox
          v-if="field.kind === 'flag'"
          class="k-edit-panel__field"
          :model-value="values[field.name] === true"
          :label="labelOf(field)"
          @update:model-value="setValue(field.name, $event === true)"
        />
        <Combobox
          v-else-if="moving && field.kind === 'reference'"
          v-model="moveText"
          v-model:value="moveValue"
          class="k-edit-panel__field"
          :label="labelOf(field)"
          :suggest="suggestTargets"
          :error="errors[field.name]"
          mono
        />
        <Select
          v-else-if="
            field.kind === 'choice' || field.kind === 'reference' || field.kind === 'anchor'
          "
          class="k-edit-panel__field"
          :model-value="selectValue(field)"
          :label="labelOf(field)"
          :options="optionsOf(field)"
          :mono="field.kind !== 'choice'"
          :required="field.required === true"
          :error="errors[field.name]"
          @update:model-value="setValue(field.name, fromSelect($event))"
        />
        <TextField
          v-else
          class="k-edit-panel__field"
          :model-value="String(values[field.name] ?? '')"
          :label="labelOf(field)"
          :mono="field.mono === true"
          :required="field.required === true"
          :error="errors[field.name]"
          :hint="hintOf(field)"
          @update:model-value="setValue(field.name, $event)"
        />
      </template>
      <template v-if="network">
        <!-- The network the VLAN is set on: read only, the refusal of the setting on it. -->
        <TextField
          :model-value="network.name"
          :label="t('views.vlan.network')"
          :hint="t('edit.vlan.networkHint')"
          :error="errors.network"
          mono
          readonly
        />
        <AttributeList :items="[{ key: 'property', label: t('views.vlan.property') }]">
          <template #property><CodeBadge :code="VLAN_PROPERTY" /></template>
        </AttributeList>
        <InlineAlert tone="info" :title="t('edit.vlan.twoOperations')">
          {{ t("edit.vlan.twoOperationsText", { network: network.name }) }}
        </InlineAlert>
      </template>
      <TextField
        v-if="applying"
        :model-value="motive"
        :label="t('edit.motive')"
        :error="errors.motive"
        required
        @update:model-value="setMotive"
      />
      <RefusalAlert
        v-if="refusal"
        :key="refusal.kind + changed.length"
        :refusal="refusal"
        :changed="changed"
      />
      <!-- Present before its sentence, so that the sentence is said. -->
      <p class="k-edit-panel__notice" role="status">{{ notice ?? "" }}</p>
      <Cluster justify="end" gap="2">
        <Button variant="ghost" @click="closeEdit()">{{ t("views.cancel") }}</Button>
        <SplitButton
          v-model:action="action"
          :actions="actions"
          :loading="busy"
          :menu-label="t('edit.chooseAction')"
          :shortcut="['Control', 'Enter']"
          @run="run"
        />
      </Cluster>
    </Stack>
  </form>
</template>

<style>
@layer components {
  .k-edit-panel__title {
    font-size: var(--k-text-md);
    font-weight: var(--k-weight-semibold);
  }

  .k-edit-panel__notice {
    color: var(--k-text-muted);
  }
}
</style>
