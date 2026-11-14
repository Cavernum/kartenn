<script setup lang="ts">
// « Ajouter un rôle » and « Modifier » on a role of a group (005, REQ-005-25, 26; spec 025,
// REQ-025-4; maquette E7): a role, by its label and key, on a scope — the whole
// organisation, or an entity, a site, a domain or a prefix chosen by its name among the
// first of its list. To the draft: `bind`, after `unbind` of the binding changed. Left open
// as the window narrows under 1024 px (the gate of writes), it keeps its input and sends
// nothing.
import { reads } from "@kartenn/sdk";
import {
  Button,
  Combobox,
  Dialog,
  InlineAlert,
  Select,
  Stack,
  filterOptions,
  type ListOption,
} from "@kartenn/ui";
import { computed, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useQueryClient } from "../data/query.ts";
import { collectionOfKind, nameOf, type Item } from "../views/collections.ts";
import { rowsOf, useKeyLabel } from "../views/data.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import { useAdminDraft } from "./draft.ts";
import {
  bindingKey,
  bindOp,
  rebind,
  SCOPE_KINDS,
  scopeOf,
  type BindingRow,
  type RoleItem,
  type ScopeKind,
} from "./model.ts";

const props = defineProps<{
  group: string;
  roles: readonly RoleItem[];
  rows: readonly BindingRow[];
  /** The binding changed; none: an addition. */
  editing?: BindingRow | undefined;
  /** The words of a binding (« reader · Domaine dc-nord »). */
  words: (role: string, scope: BindingRow["scope"]) => string;
  /** The name of the object of a scope, when known. */
  scopeName: (id: string) => string | undefined;
}>();
const open = defineModel<boolean>("open", { default: false });
const { t, locale } = useI18n();
const keyLabel = useKeyLabel();
const queryClient = useQueryClient();
const draft = useAdminDraft();
/** The form, which the button of the actions submits (Enter in a field too). */
const formId = useId();

const role = ref<string | undefined>(undefined);
const kind = ref<string | undefined>("global");
const text = ref("");
const object = ref<string | undefined>(undefined);
const tried = ref(false);
watch(open, (now) => {
  if (!now) return;
  const editing = props.editing;
  role.value = editing?.role;
  kind.value = editing?.scope.kind ?? "global";
  object.value = editing?.scope.id ?? undefined;
  text.value = editing?.scope.id ? (props.scopeName(editing.scope.id) ?? editing.scope.id) : "";
  tried.value = false;
  draft.refusal.value = null;
});
watch(kind, (now, before) => {
  if (before === undefined || now === before) return;
  object.value = undefined;
  text.value = "";
});

const roleOptions = computed<ListOption[]>(() =>
  props.roles.map((r) => ({
    value: r.key,
    label: r.labels[locale.value] ?? keyLabel("role", r.key) ?? r.key,
    detail: r.key,
  })),
);
const kindOptions = computed<ListOption[]>(() =>
  SCOPE_KINDS.map((k) => ({ value: k, label: t(`admin.bindings.scopes.${k}`) })),
);
/** The objects of the kind chosen, among the first page of its list. */
async function suggest(query: string): Promise<ListOption[]> {
  const collection = collectionOfKind(kind.value);
  if (!collection) return [];
  const data = await queryClient.fetchInfiniteQuery(reads.list(collection.path as "/v1/domains"));
  const options = (rowsOf(data) as unknown as Item[]).map((item) => ({
    value: String(item.id),
    label: nameOf(item),
    mono: true,
  }));
  return filterOptions(options, query);
}

const errors = computed(() => {
  if (!tried.value) return {};
  const required = (field: string) => t("admin.required", { field });
  if (!role.value) return { role: required(t("admin.field.role")) };
  if (kind.value !== "global" && !object.value) {
    return { object: required(t("admin.bindings.object")) };
  }
  const wanted = bindingKey(role.value, scopeOf(kind.value as ScopeKind, object.value ?? ""));
  const taken = props.rows.some(
    (r) => r !== props.editing && r.pending !== "removed" && bindingKey(r.role, r.scope) === wanted,
  );
  return taken ? { role: t("admin.bindings.already") } : {};
});

/** Why nothing is sent now: the window narrowed (or the period moved) since it opened. */
const held = computed(() => draft.gate.why.value);

async function submit(): Promise<void> {
  // Refused by the gate, read when sending: said in the dialog, the input kept.
  if (held.value) return;
  tried.value = true;
  if (Object.keys(errors.value).length > 0 || !role.value) return;
  const scope = scopeOf(kind.value as ScopeKind, object.value ?? "");
  const operations = props.editing
    ? rebind(props.editing, props.group, role.value, scope)
    : [bindOp(props.group, role.value, scope)];
  const what = t("admin.bindings.added", {
    binding: props.words(role.value, scope),
    group: props.group,
  });
  if (await draft.add(operations, what)) open.value = false;
}
</script>

<template>
  <Dialog
    v-model:open="open"
    :title="
      props.editing
        ? t('admin.bindings.editTitle', {
            binding: props.words(props.editing.role, props.editing.scope),
          })
        : t('admin.bindings.addTitle', { group: props.group })
    "
    size="md"
  >
    <form :id="formId" novalidate @submit.prevent="submit">
      <Stack gap="3">
        <Select
          :key="props.roles.length"
          v-model="role"
          :label="t('admin.field.role')"
          :options="roleOptions"
          :error="errors.role"
          required
        />
        <Select v-model="kind" :label="t('admin.bindings.scope')" :options="kindOptions" required />
        <Combobox
          v-if="kind !== 'global'"
          :key="kind ?? ''"
          v-model="text"
          v-model:value="object"
          :label="t('admin.bindings.object')"
          :hint="t('admin.bindings.objectHint')"
          :error="errors.object"
          :suggest="suggest"
          mono
          required
        />
        <InlineAlert v-if="held" tone="warn" icon="lock" :title="held" />
        <RefusalAlert v-if="draft.refusal.value" :refusal="draft.refusal.value" :note="null" />
      </Stack>
    </form>
    <template #actions>
      <Button variant="ghost" @click="open = false">{{ t("admin.cancel") }}</Button>
      <Button
        variant="primary"
        icon="draft"
        type="submit"
        :form="formId"
        :loading="draft.busy.value"
        v-bind="held ? { reason: held } : {}"
        >{{ t("admin.toDraft") }}</Button
      >
    </template>
  </Dialog>
</template>
