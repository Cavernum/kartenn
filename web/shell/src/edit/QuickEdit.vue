<script setup lang="ts">
import { DialogContent, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from "reka-ui";
import { computed, reactive, ref, shallowRef, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import { read, withQuery, type Listing } from "../data/api.ts";
import type { Collection } from "../data/collections.ts";
import { problemText } from "../i18n/index.ts";
import { CREATION, creation, initial, MODIFICATION, patch, type Values } from "./fields.ts";
import { submit, type Outcome } from "./submit.ts";

// Quick edit (REQ-007-25, design « Écriture »): create, modify or close an object by the
// route of its resource, one changeset with a motive and the `If-Match` of the read. A 412
// offers to resume on the new version with the input kept, a 202 leads to the pending
// changeset, a 422 puts the violations on their fields, a 403 keeps the input.

const props = defineProps<{
  collection: Collection;
  mode: "create" | "modify" | "close";
  /** The object read, for a modification or a closure. */
  item?: Record<string, unknown> | null;
  /** Its URL in the API (modification, closure). */
  href?: string;
  /** The `ETag` of the read. */
  etag?: string | null;
  /** A domain preselected for a creation. */
  domain?: string | null;
}>();

const emit = defineEmits<{
  /** Applied: the `Location` of a created object, the changeset. */
  applied: [location: string | null, changeset: string | null];
  /** Asks the record to read the object again (after a 412). */
  reload: [];
  cancel: [];
}>();

const { t, te } = useI18n();
const id = useId();

const fields = computed(() =>
  props.mode === "create"
    ? CREATION[props.collection]
    : props.mode === "modify"
      ? MODIFICATION[props.collection]
      : [],
);

const before = shallowRef<Values>(initial(fields.value, props.item ?? null));
const values = reactive<Values>({ ...before.value });
if (props.mode === "create" && props.domain && "domain" in values) values.domain = props.domain;
/** The fields the user changed: only they go into the patch, and survive a reload. */
const dirty = reactive(new Set<string>());
const motive = ref("");
const idempotencyKey = crypto.randomUUID();

const outcome = shallowRef<Outcome | null>(null);
const sending = ref(false);
const stale = ref(false);

// The domains a creation chooses from.
const domains = ref<{ id: string; slug: string }[]>([]);
if (fields.value.some((field) => field.kind === "domain")) {
  void read<Listing<{ id: string; slug: string }>>(
    withQuery("/v1/domains", { page_size: "1000" }),
  ).then((answer) => {
    domains.value = answer.ok ? answer.body.items : [];
    if (!values.domain && domains.value[0]) values.domain = domains.value[0].id;
  });
}

// A new version read after a 412: the untouched fields follow it, the input stays.
watch(
  () => props.item,
  (item) => {
    const next = initial(fields.value, item ?? null);
    for (const field of fields.value) {
      if (!dirty.has(field.name)) values[field.name] = next[field.name]!;
    }
    before.value = next;
  },
);

function change(name: string, value: string | boolean): void {
  values[name] = value;
  dirty.add(name);
}

const body = computed(() => {
  if (props.mode === "create") return creation(fields.value, values);
  if (props.mode === "close") return {};
  const changed = Object.fromEntries([...dirty].map((name) => [name, values[name]!]));
  return patch(fields.value, before.value, { ...before.value, ...changed });
});
const ready = computed(
  () =>
    motive.value.trim() !== "" &&
    !sending.value &&
    (props.mode !== "modify" || Object.keys(body.value).length > 0),
);

async function send(): Promise<void> {
  if (!ready.value) return;
  sending.value = true;
  outcome.value = null;
  const target =
    props.mode === "create" ? `/v1/${props.collection}` : (props.href ?? `/v1/${props.collection}`);
  // Spec 024 (REQ-024-13): applied at once by who holds apply_own, pending otherwise.
  const url = `${target}${target.includes("?") ? "&" : "?"}apply=own`;
  const result = await submit({
    method: props.mode === "create" ? "POST" : props.mode === "modify" ? "PATCH" : "DELETE",
    url,
    body: { ...body.value, motive: motive.value.trim() },
    ifMatch: props.mode === "create" ? null : props.etag,
    idempotencyKey,
  });
  sending.value = false;
  outcome.value = result;
  if (result.kind === "stale") {
    stale.value = true;
    emit("reload");
  } else if (result.kind === "applied") {
    dirty.clear();
    emit("applied", result.location, result.changeset);
  }
}

/** Back to the form on the version read again, the input kept. */
function resume(): void {
  stale.value = false;
  outcome.value = null;
}

const fieldErrors = computed(() =>
  outcome.value?.kind === "invalid" ? outcome.value.byField : {},
);
const generalErrors = computed<string[]>(() => {
  const value = outcome.value;
  if (value?.kind === "invalid") {
    return value.general.length > 0 || Object.keys(value.byField).length > 0
      ? value.general
      : [problemText({ t, te }, { type: value.type, detail: null, title: "" })];
  }
  if (value?.kind === "refused") return [problemText({ t, te }, value)];
  return [];
});

function shown(value: unknown): string {
  if (typeof value === "boolean") return t(value ? "edit.yes" : "edit.no");
  if (typeof value === "object" && value !== null && "id" in value) {
    const named = value as { slug?: unknown; id: unknown };
    return String(named.slug ?? named.id);
  }
  return value === undefined || value === null || value === "" ? "—" : String(value);
}
</script>

<template>
  <form class="k-form k-edit" :aria-labelledby="`${id}-title`" @submit.prevent="send">
    <h2 :id="`${id}-title`">{{ t(`edit.title.${mode}`) }}</h2>
    <div v-if="generalErrors.length > 0" role="alert" class="k-error">
      <p v-for="(error, index) in generalErrors" :key="index">{{ error }}</p>
    </div>
    <p v-if="outcome?.kind === 'pending'" role="status">
      {{ t("edit.pending") }}
      <RouterLink v-if="outcome.changeset" :to="`/changesets/${outcome.changeset}`">
        {{ t("edit.changeset") }}
      </RouterLink>
    </p>
    <template v-for="field in fields" :key="field.name">
      <label v-if="field.kind === 'flag'" class="k-check">
        <input
          type="checkbox"
          :checked="values[field.name] === true"
          :aria-invalid="fieldErrors[field.name] ? true : undefined"
          :aria-describedby="fieldErrors[field.name] ? `${id}-${field.name}-error` : undefined"
          @change="change(field.name, ($event.target as HTMLInputElement).checked)"
        />
        {{ t(`field.${field.name}`) }}
      </label>
      <label v-else>
        {{ t(`field.${field.name}`) }}
        <select
          v-if="field.kind === 'choice' || field.kind === 'domain'"
          :value="values[field.name]"
          :aria-invalid="fieldErrors[field.name] ? true : undefined"
          :aria-describedby="fieldErrors[field.name] ? `${id}-${field.name}-error` : undefined"
          @change="change(field.name, ($event.target as HTMLSelectElement).value)"
        >
          <template v-if="field.kind === 'choice'">
            <option v-for="option in field.options" :key="option" :value="option">
              {{ option }}
            </option>
          </template>
          <template v-else>
            <option v-for="d in domains" :key="d.id" :value="d.id">{{ d.slug }}</option>
          </template>
        </select>
        <input
          v-else
          type="text"
          :value="values[field.name]"
          :required="field.kind === 'text' && field.required"
          :aria-invalid="fieldErrors[field.name] ? true : undefined"
          :aria-describedby="fieldErrors[field.name] ? `${id}-${field.name}-error` : undefined"
          @input="change(field.name, ($event.target as HTMLInputElement).value)"
        />
      </label>
      <p
        v-if="fieldErrors[field.name]"
        :id="`${id}-${field.name}-error`"
        class="k-error k-field-error"
      >
        {{ fieldErrors[field.name]!.join(" ") }}
      </p>
    </template>
    <label>
      {{ t("edit.motive") }}
      <textarea v-model="motive" required rows="2" />
    </label>
    <p class="k-toolbar">
      <button type="submit" :disabled="!ready">{{ t(`edit.submit.${mode}`) }}</button>
      <button type="button" @click="emit('cancel')">{{ t("edit.cancel") }}</button>
    </p>
  </form>
  <DialogRoot :open="stale" @update:open="(open) => !open && resume()">
    <DialogPortal>
      <DialogOverlay class="k-overlay" />
      <DialogContent class="k-dialog" :aria-describedby="`${id}-stale`">
        <DialogTitle>{{ t("edit.stale.title") }}</DialogTitle>
        <p :id="`${id}-stale`">{{ t("edit.stale.detail") }}</p>
        <table v-if="mode === 'modify'" class="k-table">
          <thead>
            <tr>
              <th scope="col">{{ t("edit.stale.field") }}</th>
              <th scope="col">{{ t("edit.stale.current") }}</th>
              <th scope="col">{{ t("edit.stale.yours") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="field in fields" :key="field.name">
              <th scope="row">{{ t(`field.${field.name}`) }}</th>
              <td>{{ shown(item?.[field.name]) }}</td>
              <td>{{ dirty.has(field.name) ? shown(values[field.name]) : "—" }}</td>
            </tr>
          </tbody>
        </table>
        <p class="k-toolbar">
          <button type="button" @click="resume">{{ t("edit.stale.resume") }}</button>
        </p>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
