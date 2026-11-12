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
// refusal of the setting said on the field of the network. « Modifier » of an operation of a
// draft (task 14l, Q-021-17): the same form prefilled from the operation — a creation, or the
// modification over the object read —, which replaces it at its place under `If-Match`
// (`replaceInDraft`), the draft's simulation asked again by its new version.
import { addToActiveDraft, replaceInDraft } from "@kartenn/sdk/host";
import {
  changesets,
  keys,
  queryClient,
  reads,
  write,
  type components,
  type Entry,
  type ListPath,
  type Operation,
  type Submission,
} from "@kartenn/sdk";
import {
  AttributeList,
  Button,
  Checkbox,
  Cluster,
  CodeBadge,
  Combobox,
  filterOptions,
  Heading,
  InlineAlert,
  Select,
  Skeleton,
  SplitButton,
  Stack,
  TextField,
  type ListOption,
  type Suggest,
} from "@kartenn/ui";
import { computed, nextTick, reactive, ref, useTemplateRef, watch, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery, useQuery } from "../../data/query.ts";
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
import { cacheSuggest } from "../filters.ts";
import { operationLine } from "../../draft/operations.ts";
import { failed, isConflict, useWriteAction } from "./action.ts";
import {
  ANCHOR_KINDS,
  changesOf,
  checkValue,
  draftedCreations,
  editOfOperation,
  editRight,
  EDITS,
  initialValues,
  localText,
  patchOf,
  textOf,
  type EditField,
  type Values,
} from "./fields.ts";
import { VLAN_PROPERTY } from "../parts/vlan.ts";
import type { Refusal } from "./inline.ts";
import { useMoves, type MoveOutcome } from "./moves.ts";
import { closeEdit, type FormRequest } from "./panel.ts";
import RefusalAlert from "./RefusalAlert.vue";
import {
  fieldErrors,
  NETWORK_VLAN_EDITS,
  nextFreeVid,
  vlanForNetwork,
  vlanForNetworkInDraft,
} from "./vlan.ts";

const props = defineProps<{ request: FormRequest }>();
const { t, te } = useI18n();
const keyLabel = useKeyLabel();

const chosenKind = ref<CollectionId>(props.request.collection);
const collection = computed(() => collectionById(chosenKind.value)!);
const kindWords = computed(() => keyLabel("kind", collection.value.type) ?? collection.value.kind);
const creating = computed(() => props.request.mode === "create");
const moving = computed(() => props.request.mode === "move");
const objectId = computed(() => (props.request.mode === "create" ? null : props.request.id));
/** « Modifier » of an operation of a draft: where it is, the operation, its line. */
const replacing = computed(() => {
  const slot = props.request.mode === "move" ? undefined : props.request.replace;
  const operation = slot?.draft.data.operations?.[slot.index];
  return slot && operation
    ? { ...slot, operation, line: operationLine(operation), edit: editOfOperation(operation) }
    : null;
});
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

// The object modified, read at the present (the same entry as its record; under `valid_at`
// alone, the version the dated draft writes on).
const record = useQuery(() => ({
  ...reads.object<Item>(collection.value.kind, objectId.value ?? "-"),
  enabled: objectId.value !== null,
}));
const item = computed(() => (record.data.value as Entry<Item> | undefined)?.data ?? null);
const name = computed(() => (item.value ? nameOf(item.value) : ""));

/**
 * The read of the nodes a move chooses among (the entities or the sites): the ancestors of
 * each target, for the check of a cycle; the targets themselves are suggested by `q`.
 */
const nodesRead = useInfiniteQuery(() => ({
  ...reads.list((collection.value.id === "sites" ? "/v1/sites" : "/v1/entities") as ListPath),
  enabled: moving.value,
}));
// A move reads the pages after the first (ten at most): the ancestors of each target known
// for the check of a cycle.
const MOVE_PAGES = 10;
watchEffect(() => {
  const nodes = nodesRead;
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
 * The option « Aucun » of an optional reference (a root): the empty string is the field's
 * « nothing taken », so the option has a value of its own, read back as `""`.
 */
const NONE = "none";
const fromOption = (value: string | undefined) => (value === NONE ? "" : (value ?? ""));
/**
 * The objects an earlier operation of the draft creates (a local name, 021/A13), offered
 * beside those read to the references of an operation replaced (task 14l): named by what
 * that operation names, said « opération n du brouillon ».
 */
const drafted = computed(() => {
  const slot = replacing.value;
  return slot ? draftedCreations(slot.draft.data.operations ?? [], slot.index) : [];
});
function draftedOptions(
  target: CollectionId,
  value: (local: number) => string,
  kind?: string,
): ListOption[] {
  return drafted.value
    .filter((d) => d.collection === target)
    .map((d) => ({
      value: value(d.local),
      label: operationLine(d.operation).subject ?? `#${d.local}`,
      detail: kind
        ? t("edit.replace.draftedOf", { kind, n: d.index + 1 })
        : t("edit.replace.drafted", { n: d.index + 1 }),
      mono: true,
    }));
}
/** The options of a closed list of the contract (nature, usage…), in its words. */
function choicesOf(field: EditField): ListOption[] {
  return (field.options ?? []).map((key) => {
    const words = `views.value.${field.name}.${key}`;
    return { value: key, label: te(words) ? t(words) : key, detail: key };
  });
}
const isReference = (field: EditField) => field.kind === "reference" || field.kind === "anchor";
/** The objects of the draft a reference may name (an operation replaced, task 14l). */
function draftedOf(field: EditField): ListOption[] {
  if (field.kind !== "anchor") return field.target ? draftedOptions(field.target, localText) : [];
  return ANCHOR_KINDS.flatMap((kind) => {
    const of = collectionOfKind(kind)!;
    const words = keyLabel("kind", of.type) ?? kind;
    return draftedOptions(of.id, (local) => `${kind}:${localText(local)}`, words);
  });
}
/** A move offers the valid targets only (REQ-021-32): the others are not offered at all. */
const validTarget = (id: string | null, label: string) =>
  !moving.value || moves.canMove(movedNode.value, id === null ? null : { id, label }).ok;

// A reference (the domain, the parent of an entity or a site, the group of a VLAN, the
// anchor of a group) is a `Combobox` (task 12j): its suggestions are asked of its collection
// by `q` as the text is typed (`cacheSuggest`, the first page of the answer), so that any
// object is found, not only those of a first page read; beside them, « Aucun » for an
// optional one, and the objects of the draft an operation replaced may name (14l).
/** The words of the objects named so far, by value: suggested, read, of the draft. */
const labels = reactive(new Map<string, string>());
async function suggestionsOf(
  field: EditField,
  query: string,
  signal: AbortSignal,
): Promise<ListOption[]> {
  const q = query.trim();
  const read = async (id: CollectionId) =>
    (await cacheSuggest(
      collectionById(id)!.path as ListPath,
      q === "" ? {} : { q },
      signal,
    )) as unknown as Item[];
  let found: ListOption[];
  if (field.kind === "anchor") {
    const lists = await Promise.all(
      ANCHOR_KINDS.map(async (kind) => {
        const of = collectionOfKind(kind)!;
        const words = keyLabel("kind", of.type) ?? kind;
        return (await read(of.id)).map((i) => ({
          value: `${kind}:${String(i.id)}`,
          label: nameOf(i),
          detail: words,
          mono: true,
        }));
      }),
    );
    found = lists.flat();
  } else {
    found = (await read(field.target!))
      .filter((i) => validTarget(String(i.id), nameOf(i)))
      .map((i) => ({ value: String(i.id), label: nameOf(i), mono: true }));
  }
  const none =
    field.kind === "reference" && !field.required && validTarget(null, t("edit.none"))
      ? [{ value: NONE, label: t("edit.none") }]
      : [];
  const options = [...filterOptions(none, q), ...found, ...filterOptions(draftedOf(field), q)];
  for (const option of options) labels.set(option.value, option.label);
  return options;
}
/** The function of suggestions of each reference, the same while its field is shown. */
const suggests = computed(
  () =>
    new Map(
      fields.value
        .filter(isReference)
        .map((field): [string, Suggest] => [
          field.name,
          (query, signal) => suggestionsOf(field, query, signal),
        ]),
    ),
);
/** The words of a value of a reference: « Aucun », what names it, else its identifier. */
function referenceLabel(field: EditField, value: string): string {
  if (value === "") return field.required ? "" : t("edit.none");
  return labels.get(value) ?? draftedOf(field).find((o) => o.value === value)?.label ?? value;
}

/** A reference's field: its text, and the value taken from the suggestions, if one is. */
interface Pick {
  text: string;
  value: string | undefined;
}
const picks = ref<Record<string, Pick>>({});
/** The words a reference read gives of its object (a summary's slug), `""` if none. */
function summaryName(field: EditField, read: unknown): string {
  const summary = field.kind === "anchor" ? Object.values(read ?? {})[0] : read;
  if (typeof summary !== "object" || summary === null) return "";
  const named = nameOf(summary as Item);
  return named === textOf(summary) ? "" : named;
}
/**
 * The field of a reference as the form opens: the words of its value — read with the object
 * (a summary's slug), of the draft, else read by its identifier (a value given beforehand,
 * an anchor or a group read without their words): the field empty until then, never an
 * opaque identifier read out, which shows only if the object cannot be read.
 */
function pickOf(field: EditField, value: string, read: unknown): Pick {
  if (value === "") {
    // A root (« Changer de parent… »): « Aucun » taken; a creation starts empty.
    return moving.value && !field.required
      ? { text: t("edit.none"), value: NONE }
      : { text: "", value: undefined };
  }
  const named = textOf(read) === value ? summaryName(field, read) : "";
  if (named !== "") labels.set(value, named);
  const text = referenceLabel(field, value);
  if (text !== value || value.includes("local:")) return { text, value };
  void readLabel(field, value);
  return { text: "", value };
}
/** The words of a reference unknown so far, from its object read by its identifier. */
async function readLabel(field: EditField, value: string): Promise<void> {
  const [kind, id] =
    field.kind === "anchor"
      ? [value.slice(0, value.indexOf(":")), value.slice(value.indexOf(":") + 1)]
      : [collectionById(field.target!)!.kind, value];
  let label: string;
  try {
    label = nameOf((await queryClient.fetchQuery(reads.object<Item>(kind, id))).data);
    labels.set(value, label);
  } catch {
    label = value; // Unread: its identifier, the only words left.
  }
  // Shown unless the user typed meanwhile (the value then let go by the Combobox).
  const pick = picks.value[field.name];
  if (pick?.value === value && pick.text === "") {
    picks.value = { ...picks.value, [field.name]: { text: label, value } };
  }
}
/** A text typed: no value taken any more until a suggestion is (the Combobox clears it). */
function typed(field: EditField, text: string): void {
  const pick = picks.value[field.name];
  picks.value = { ...picks.value, [field.name]: { text, value: pick?.value } };
}
/** A suggestion taken, or the value let go by a text typed: the field's value. */
function picked(field: EditField, value: string | undefined): void {
  const pick = picks.value[field.name];
  picks.value = { ...picks.value, [field.name]: { text: pick?.text ?? "", value } };
  setValue(field.name, fromOption(value));
}

// « Changer de parent… »: the node moved, its parent read, and the ancestors of each target
// as the pages read show them (a cycle beyond them is the server's to refuse).
const parentIds = computed(() => {
  const items = rowsOf(nodesRead.data.value as never) as unknown as Item[];
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
/** The form as it opened: an operation replaced, its own values over the object read. */
const opened = ref<Values>({});
const version = ref<string | null>(null);
const filled = ref(false);
function fill(): void {
  const start = initialValues(
    fields.value,
    item.value,
    props.request.mode === "create" ? (props.request.preset ?? {}) : {},
  );
  values.value = { ...start, ...replacing.value?.edit?.values };
  before.value = { ...start };
  opened.value = { ...values.value };
  version.value = typeof item.value?.version === "string" ? item.value.version : null;
  picks.value = Object.fromEntries(
    fields.value
      .filter(isReference)
      .map((field) => [
        field.name,
        pickOf(field, String(values.value[field.name] ?? ""), item.value?.[field.name]),
      ]),
  );
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
/** An input of the user: a new submission, under a new idempotency key (`action.ts`). */
function setValue(name: string, value: string | boolean): void {
  values.value = { ...values.value, [name]: value };
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
        group: referenceLabel(
          fields.value.find((f) => f.name === "group")!,
          vidProposed.value,
        ),
      })
    : undefined;

/**
 * The edit permissions the action needs: `X.edit` of its kind; a VLAN set on a network also
 * writes the network and its property `core.vlan`. To apply, the approval of each.
 */
const editRights = computed(() => [
  editRight(collection.value.type),
  ...(network.value ? NETWORK_VLAN_EDITS : []),
]);
/** A move's destination, in its words: the target chosen, or the root. */
const destination = computed(() => {
  if (!moving.value) return "";
  const parent = String(values.value.parent ?? "");
  if (parent === "") return t("edit.move.root");
  return referenceLabel(fields.value[0]!, parent);
});
// The split button, the generic motive prefilled, the idempotency key and the outcome said
// (`action.ts`): the key follows the body sent — an input of the user, another kind, the
// version read again after a conflict, a new one.
const {
  action,
  actions,
  applying,
  motive,
  setMotive,
  idempotencyKey,
  busy,
  notice,
  refusal,
  ...writing
} = useWriteAction({
  rights: () => editRights.value,
  motive: () => {
    const words = { kind: kindWords.value, name: name.value, to: destination.value };
    return network.value
      ? t("edit.vlan.motiveFor", { network: network.value.name })
      : creating.value
        ? t("edit.motiveCreate", words)
        : t(moving.value ? "edit.move.motive" : "edit.motiveModify", words);
  },
  words: {
    draft: "edit.toDraft",
    apply: "edit.apply",
    past: "edit.why.past",
    ...(replacing.value
      ? { saved: (what: string) => t("edit.replace.done", { n: replacing.value!.index + 1, what }) }
      : {}),
  },
  refused,
});
/**
 * What the refusal of a replacement says: the draft gone (404), or changed so that the
 * operation is no longer at its place (412, read again by the SDK).
 */
const refusalWords = computed(() => {
  const answer = refusal.value;
  if (!replacing.value || !answer) return {};
  if (answer.kind === "failed" && answer.problem?.status === 404) {
    return { title: t("drafts.goneRefused"), note: t("edit.replace.goneText") };
  }
  return answer.kind === "conflict"
    ? { title: t("drafts.changed"), note: t("edit.replace.changedText") }
    : {};
});
/** « Remplacer dans le brouillon »: the refusal of the draft action, if any (rights). */
const replaceRefusal = computed(() => {
  const draft = actions.value.find((a) => a.id === "draft");
  return draft?.forbidden
    ? { forbidden: draft.forbidden }
    : draft?.reason
      ? { reason: draft.reason }
      : {};
});

const title = computed(() => {
  const request = props.request;
  const slot = replacing.value;
  if (slot) {
    return t("edit.replace.title", {
      n: slot.index + 1,
      op: slot.line.op,
      subject: slot.line.subject ?? "",
    });
  }
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
/** The focus to the first field refused (local check, 422), its error read with it. */
async function toFirstError(): Promise<void> {
  await nextTick();
  root.value?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus();
}

const labelOf = (field: EditField) => t(`views.field.${field.name}`);
const shownText = (field: EditField, value: unknown) => {
  const text = textOf(value);
  if (field.kind === "flag") return t(`views.value.boolean.${value === true}`);
  if (isReference(field)) {
    return (
      (labels.has(text) ? labels.get(text) : summaryName(field, value)) ||
      referenceLabel(field, text) ||
      "—"
    );
  }
  return text || "—";
};
/** After a 412: each field whose value read again differs from the input. */
const changed = computed(() => {
  // An operation replaced: a 412 is of the draft, whose operation moved (`refusalWords`).
  const current =
    refusal.value?.kind === "conflict" && !replacing.value
      ? (refusal.value.current?.data as Item)
      : null;
  if (!current) return [];
  return fields.value
    .filter((field) => textOf(current[field.name]) !== textOf(values.value[field.name]))
    .map((field) => ({
      label: labelOf(field),
      current: shownText(field, current[field.name]),
      input: shownText(field, values.value[field.name]),
    }));
});

// A modification replaced keeps the version it was written on (Q-021-20): an object changed
// since is said, with the fields that moved, and the operation stays in conflict until it
// is taken up at the version read now — an action of its own, never silent.
/** The version the operation replaced was written on, if it names one. */
const originalExpected = computed(() => {
  const expected = (replacing.value?.operation as { expected?: unknown } | undefined)?.expected;
  return typeof expected === "string" ? expected : null;
});
/** « Reprendre sur la version actuelle » chosen. */
const rebased = ref(false);
/** The object changed since the operation was written. */
const stale = computed(
  () =>
    replacing.value !== null &&
    !creating.value &&
    originalExpected.value !== null &&
    version.value !== null &&
    originalExpected.value !== version.value,
);
type Version = components["schemas"]["Version"];
const history = useInfiniteQuery(() => ({
  ...reads.subPages<Version>(collection.value.kind, objectId.value ?? "-", "history"),
  enabled: stale.value,
}));
/** The fields that moved since that version: as they were, as they are (first page read). */
const moved = computed(() => {
  const current = item.value;
  const then = rowsOf(history.data.value as never)
    .map((v) => (v as unknown as Version | undefined)?.object as unknown as Item | undefined)
    .find((object) => object?.version === originalExpected.value);
  if (!current || !then) return [];
  return fields.value
    .filter((field) => textOf(then[field.name]) !== textOf(current[field.name]))
    .map((field) => ({
      message: t("edit.replace.movedField", {
        field: labelOf(field),
        then: shownText(field, then[field.name]),
        now: shownText(field, current[field.name]),
      }),
    }));
});
/** Takes the operation up at the version read now: written on it once replaced. */
async function rebase(): Promise<void> {
  rebased.value = true;
  writing.tell(t("edit.replace.rebased", { version: version.value }));
  // The alert goes with its button: the focus to the first field (2.4.3).
  await nextTick();
  root.value?.querySelector<HTMLElement>(".k-edit-panel__field :is(input, button)")?.focus();
}

function checked(): boolean {
  const found: Record<string, string> = {};
  for (const field of fields.value) {
    // A text typed and no suggestion taken: nothing is guessed; a move takes a target or
    // « Aucun » from the list of the valid ones, never an empty field.
    const pick = isReference(field) ? picks.value[field.name] : undefined;
    if (pick && pick.value === undefined && (pick.text.trim() !== "" || moving.value)) {
      found[field.name] = moving.value
        ? t("edit.move.choose")
        : t("edit.check.pick", { field: labelOf(field) });
      continue;
    }
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

/** A refusal of the server, kept in the panel with the input: on its fields, or read again. */
function refused(answer: Refusal): void {
  if (answer.kind === "invalid") {
    // Each violation on the field its pointer names: a member of the patch, or of the one
    // operation of a creation (`/operations/0/prefix` → « Valeur »); for a VLAN created for
    // a network, the setting of `core.vlan` (`/operations/1/…`) on the field of the network.
    errors.value = fieldErrors(
      collection.value.id,
      new Set(fields.value.map((field) => field.name)),
      [...Object.values(answer.byField).flat(), ...answer.general],
      network.value !== undefined,
    );
    void toFirstError();
  }
  // An operation replaced: the 412 is of the draft, read again by the SDK; nothing here.
  if (replacing.value) return;
  // Applying again writes on the version read again; the input stays. A 412 gives it; a
  // changeset refused for a version no longer current (a move) does not: read again here.
  const current = answer.kind === "conflict" ? (answer.current?.data as Item | undefined) : null;
  if (current) readAt(current);
  else if (isConflict(answer)) void readAgain();
}
function readAt(current: Item): void {
  before.value = initialValues(fields.value, current);
  version.value = typeof current.version === "string" ? current.version : version.value;
}
async function readAgain(): Promise<void> {
  await record.refetch();
  if (item.value) readAt(item.value);
}

async function run(id: string = action.value): Promise<void> {
  // Checked again when it is written: the right, and nothing off the present (`action.ts`).
  if (!writing.begin(id)) return;
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
  if (replacing.value) {
    await replace(what);
    return;
  }
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
    writing.tell(t("edit.unchanged"));
    return;
  }
  // A VLAN for a network: its creation and the setting of `core.vlan` by its local name —
  // in a draft, one free in it, made by the SDK from the draft at the version it writes
  // (again after a 412); applied at once, the first.
  if (id === "draft") {
    const op = creating.value
      ? edit.create(values.value)
      : edit.update(objectId.value!, version.value, changes);
    const operations = network.value ? vlanForNetworkInDraft(op, network.value) : [op];
    await writing.write(what, () => addToActiveDraft(operations));
    return;
  }
  if (creating.value) {
    const op = edit.create(values.value);
    const submission: Submission = {
      motive: motive.value.trim(),
      operations: network.value ? vlanForNetwork(op, network.value, 0) : [op],
    };
    const key = idempotencyKey(submission);
    await writing.write(what, () => changesets.submit(submission, key, { approvalSelf: true }));
    return;
  }
  await writing.write(what, () =>
    write.quick({
      key: keys.object(collection.value.kind, objectId.value!),
      patch: patchOf(fields.value, changes),
      motive: motive.value.trim(),
      ...(version.value ? { ifMatch: `"${version.value}"` } : {}),
    }),
  );
}

/**
 * « Modifier » of an operation of a draft: the operation the form gives, in its place
 * (`replaceInDraft`, under `If-Match` of the draft as its line was read). A creation keeps
 * its local name (021/A13: what refers to it still does); a modification is written over
 * the object read now (its version as `expected`), only what differs from it. Nothing
 * typed: said, nothing sent; the object already as typed: said, the removal to do instead.
 */
async function replace(what: string): Promise<void> {
  const slot = replacing.value!;
  const typed = changesOf(fields.value, opened.value, values.value);
  if (Object.keys(typed).length === 0 && !rebased.value) {
    writing.tell(t("edit.unchanged"));
    return;
  }
  const edit = EDITS[collection.value.id];
  let op: Operation;
  if (creating.value) {
    const local = (slot.operation as { local?: unknown }).local;
    op = { ...edit.create(values.value), ...(typeof local === "number" ? { local } : {}) };
  } else if (rebased.value) {
    // Taken up at the version read now: only what differs from the object as it is.
    const changes = changesOf(fields.value, before.value, values.value);
    if (Object.keys(changes).length === 0) {
      writing.tell(t("edit.replace.same"));
      return;
    }
    op = edit.update(objectId.value!, version.value, changes);
  } else {
    // At the version the operation was written on (Q-021-20): what it wrote, and what was
    // typed besides; still in conflict, as before, if the object changed since.
    const names = new Set([...Object.keys(slot.edit?.values ?? {}), ...Object.keys(typed)]);
    const changes = Object.fromEntries(
      Object.entries(values.value).filter(([name]) => names.has(name)),
    );
    op = edit.update(objectId.value!, originalExpected.value, changes);
  }
  await writing.write(what, () => replaceInDraft(slot.draft, slot.index, op));
}

/**
 * « Changer de parent… »: the node under `parent` (`""`: a root), to the draft or applied now;
 * nothing is shown at its new place before the answer (X33). The simulation of the draft,
 * or the server, may still refuse it: the reason and its code on the field.
 */
async function moveTo(parent: string, now: boolean, what: string): Promise<void> {
  // A target taken from the list of the valid ones (`checked`): nothing is guessed.
  if (parent === (before.value.parent ?? "")) {
    writing.tell(t("edit.unchanged"));
    return;
  }
  busy.value = true;
  const label = referenceLabel(fields.value[0]!, parent);
  // A rejection settles as a refusal: the button given back, the problem said.
  const outcome: MoveOutcome | Refusal = await moves
    .move(
      { node: movedNode.value, target: parent ? { id: parent, label } : null },
      { now, motive: motive.value.trim(), idempotencyKey },
    )
    .catch(failed);
  const refuse = (check: { reason: string; code?: string }) => {
    busy.value = false;
    errors.value = { parent: check.code ? `${check.reason} (${check.code})` : check.reason };
    void toFirstError();
  };
  if (outcome.kind === "unchanged") {
    busy.value = false;
    writing.tell(t("edit.unchanged"));
  } else if (outcome.kind === "refused") refuse(outcome.check);
  else if (outcome.kind === "drafted") {
    const state = moves.moveState(movedNode.value);
    if (outcome.verdict === "unknown") {
      // In the draft, its simulation not done: the problem said, the panel kept.
      busy.value = false;
      notice.value = t("edit.move.unsimulated", { what });
      refusal.value = outcome.failure ?? null;
    } else if (outcome.verdict === "valid" || typeof state !== "object" || state.refused.ok) {
      writing.settle({ kind: "saved" }, what);
    } else {
      // In the draft, but its simulation refuses it: its operation kept there, marked
      // refused; another target chosen here replaces it.
      notice.value = t("edit.move.keptRefused", { what });
      refuse(state.refused);
    }
  } else writing.settle(outcome, what);
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
      <Heading :level="2">{{ title }}</Heading>
      <p v-if="replacing" class="k-edit-panel__lead">
        {{ t("edit.replace.lead", { title: replacing.draft.data.title }) }}
      </p>
      <InlineAlert
        v-if="stale && !rebased"
        tone="warn"
        :title="t('edit.replace.stale')"
        :items="moved"
      >
        <Stack gap="2">
          <p>
            {{ t("edit.replace.staleText", { then: originalExpected, now: version }) }}
          </p>
          <Cluster>
            <Button size="sm" @click="rebase">{{ t("edit.replace.rebase") }}</Button>
          </Cluster>
        </Stack>
      </InlineAlert>
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
          v-else-if="isReference(field)"
          class="k-edit-panel__field"
          :model-value="picks[field.name]?.text ?? ''"
          :value="picks[field.name]?.value"
          :label="labelOf(field)"
          :suggest="suggests.get(field.name)!"
          :hint="t('edit.referenceHint')"
          :required="field.required === true"
          :error="errors[field.name]"
          mono
          @update:model-value="typed(field, $event)"
          @update:value="picked(field, $event)"
        />
        <Select
          v-else-if="field.kind === 'choice'"
          class="k-edit-panel__field"
          :model-value="String(values[field.name] ?? '')"
          :label="labelOf(field)"
          :options="choicesOf(field)"
          :required="field.required === true"
          :error="errors[field.name]"
          @update:model-value="setValue(field.name, $event ?? '')"
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
        v-bind="refusalWords"
      />
      <!-- Present before its sentence, so that the sentence is said. -->
      <p class="k-edit-panel__notice" role="status">{{ notice ?? "" }}</p>
      <Cluster justify="end" gap="2">
        <Button variant="ghost" @click="closeEdit()">{{ t("views.cancel") }}</Button>
        <!-- An operation of a draft replaced: to that draft only, nothing applied at once. -->
        <Button
          v-if="replacing"
          variant="primary"
          icon="draft"
          :loading="busy"
          :shortcut="['Control', 'Enter']"
          v-bind="replaceRefusal"
          @click="run('draft')"
          >{{ t("edit.replace.run") }}</Button
        >
        <SplitButton
          v-else
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
  .k-edit-panel__notice,
  .k-edit-panel__lead {
    color: var(--k-text-muted);
  }
}
</style>
