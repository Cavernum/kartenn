<script setup lang="ts">
// The record of an object of the referential (REQ-021-9; design 021 « Écrans » : fiche en
// panneaux; maquette E2): its name and kind, the tabs of its sections — resolved properties
// and their explanation, content (a domain, a prefix), history, anomalies — the tab in the
// URL (`?tab=history`); beside them its attributes and, for a prefix, its occupancy and
// next free value. Read at the dates of the page; an object out of reach (404, 403) says
// so with its codes, never a blank page. « Modifier » (`e`) opens the edit panel of the
// kind (task 12); « Changer de parent… », in the menu of an entity or a site, the same panel
// on the parent alone (task 24, REQ-021-32); « Clore… », last in the menu of every kind, its
// closure in the same place (`ClosePanel`, task 11g, 021/A31).
import { reads } from "@kartenn/sdk";
import {
  AttributeList,
  Button,
  CodeBadge,
  KeyLabel,
  Menu,
  Skeleton,
  Split,
  Stack,
  StatusBadge,
  Tabs,
  type MenuItem,
  type TabItem,
} from "@kartenn/ui";
import { computed, nextTick, ref, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { useQuery } from "../data/query.ts";
import ExtensionElement from "../plugins/ExtensionElement.vue";
import PluginAction from "../plugins/PluginAction.vue";
import { labelOf, type Placed } from "../plugins/registry.ts";
import {
  actionItems,
  objectRef,
  placed,
  placeId,
  tabItems,
  useExtensions,
} from "../plugins/slots.ts";
import { useSettings } from "../frame/settings.ts";
import KLink from "../prefetch/KLink.vue";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useTime, writesOf } from "../time/index.ts";
import { EDITS, editRight } from "./edit/fields.ts";
import { whyWords } from "./edit/inline.ts";
import { moveEntryRefusal } from "./edit/moves.ts";
import { openEdit } from "./edit/panel.ts";
import {
  anchorOf,
  fieldText,
  isSummary,
  nameOf,
  pageOfHref,
  type Collection,
  type Item,
} from "./collections.ts";
import ProblemState from "../problems/ProblemState.vue";
import { useKeyLabel } from "./data.ts";
import { keepRecordName } from "./names.ts";
import ViewPanel from "./parts/ViewPanel.vue";
import { VLAN_STATES } from "./parts/vlan.ts";
import AnomaliesTab from "./record/AnomaliesTab.vue";
import ContentTab from "./record/ContentTab.vue";
import GroupVlansTab from "./record/GroupVlansTab.vue";
import HistoryTab from "./record/HistoryTab.vue";
import NetworkVlanTab from "./record/NetworkVlanTab.vue";
import ObjectName from "./record/ObjectName.vue";
import OccupancyPanel from "./record/OccupancyPanel.vue";
import PropertiesTab from "./record/PropertiesTab.vue";
import VlanNetworksTab from "./record/VlanNetworksTab.vue";
import VlanOccupancyPanel from "./record/VlanOccupancyPanel.vue";

const props = defineProps<{ collection: Collection; id: string }>();
const { t, te } = useI18n();
const route = useRoute();
const router = useRouter();
const time = useTime();
/** What may be written at the date read (`time/`): the draft, at once (the present). */
const writes = computed(() => writesOf(time.value));
const keyLabel = useKeyLabel();
const settings = useSettings();

const record = useQuery(() =>
  reads.object(props.collection.kind, props.id, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  }),
);
const item = computed(() => record.data.value?.data as Item | undefined);
const name = computed(() => (item.value ? nameOf(item.value) : props.id));
watchEffect(() => {
  if (item.value) keepRecordName(props.id, nameOf(item.value));
});
const kindWords = computed(() => keyLabel("kind", props.collection.type) ?? props.collection.kind);

/** A key of a closed list of the core in words (`network` → « Réseau »). */
const valueWords = (field: string, value: unknown) => {
  const key = `views.value.${field}.${String(value)}`;
  return te(key) ? t(key) : null;
};
const detail = computed(() => {
  const data = item.value;
  if (!data) return undefined;
  const nature = data.nature ?? data.usage ?? data.status;
  const field =
    data.nature !== undefined ? "nature" : data.usage !== undefined ? "usage" : "status";
  return [kindWords.value, nature ? (valueWords(field, nature) ?? String(nature)) : null]
    .filter(Boolean)
    .join(" · ");
});

/** The kinds that hold others: a domain its prefixes, a prefix its children. */
const contentKind = computed(() =>
  props.collection.kind === "domain" || props.collection.kind === "prefix"
    ? props.collection.kind
    : null,
);
const hasContent = computed(() => contentKind.value !== null);

// The places of the plugins on the record of this kind (REQ-021-13, task 16): their tabs
// after the core's, their panels beside its own, their actions in the menu; each element
// given the object, its error kept in its place.
const tr = (key: string) => t(key);
const pluginTabs = useExtensions("tab", () => props.collection.kind);
const pluginPanels = useExtensions("panel", () => props.collection.kind);
const pluginActions = useExtensions("action", () => props.collection.kind);
const object = computed(() =>
  objectRef(
    props.collection.kind,
    props.id,
    String(
      (item.value?.links as { self?: unknown } | undefined)?.self ??
        `/v1/${props.collection.id}/${props.id}`,
    ),
  ),
);
/**
 * The menu of actions: « Changer de parent… » of an entity or a site, those of the plugins,
 * then « Clore… » (021/A31, task 11g), last, apart.
 */
const MOVE = "core:move";
const CLOSE = "core:close";
const movable = computed(() => EDITS[props.collection.id].move !== undefined);
const actions = computed<MenuItem[]>(() => {
  const refused = {
    ...("reason" in editBlocked.value ? { disabled: true } : {}),
    ...editBlocked.value,
  };
  // A change of parent at the present only (`moves.ts`): under `valid_at` alone too.
  const moveRefused = moveEntryRefusal(refused, writes.value, t("edit.move.dated"));
  const own: MenuItem[] = movable.value
    ? [{ id: MOVE, label: t("edit.move.action"), icon: "move", ...moveRefused }]
    : [];
  // The plugins' entries open with a separator (`actionItems`): none at the top of the menu.
  const theirs = actionItems(pluginActions.value, tr);
  const lead = theirs[0]?.kind === "separator" ? theirs.slice(1) : theirs;
  const before = own.length > 0 ? [...own, ...theirs] : lead;
  return [
    ...before,
    ...(before.length > 0 ? [{ kind: "separator" } as const] : []),
    { id: CLOSE, label: t("edit.close.action"), icon: "trash", danger: true, ...refused },
  ];
});

// « Modifier » (`e`) and the actions: the edit panel of the kind (task 12, REQ-021-36), with
// `X.edit`; refused otherwise, visible, with the permission or the reason (REQ-021-37). Under
// `as_of`, nothing is written; under `valid_at` alone, the panel opens to the dated draft
// only (REQ-021-5, 007/W9; `action.ts`).
const session = useSession();
const editPermission = computed(() => editRight(props.collection.type));
const mayEdit = computed(() => session.value.rights.has(editPermission.value));
// Off the present before a permission missing, as the cells and the keys say it (task 12k).
const blockedBy = (permission: string) =>
  !writes.value.draft
    ? { reason: t("edit.why.past") }
    : !session.value.rights.has(permission)
      ? { forbidden: permission }
      : {};
const editBlocked = computed(() => blockedBy(editPermission.value));
function modify(): void {
  openEdit({ mode: "edit", collection: props.collection.id, id: props.id });
}
// Refused, `e` says why by the status of the record, as « Modifier » says it on its button
// (`blockedBy`: off the present first, `view.ts`).
const said = ref("");
useViewAction("edit", {
  label: () => t("edit.editThis", { kind: kindWords.value, name: name.value }),
  allowed: () => mayEdit.value && item.value !== undefined,
  why: () =>
    item.value === undefined
      ? undefined
      : whyWords({ kind: "forbidden", permission: editPermission.value }, t),
  // Emptied first: the same sentence twice is said twice.
  say: (text) => {
    said.value = "";
    void nextTick(() => (said.value = text));
  },
  run: modify,
  dated: true,
});
const acting = ref<Placed | null>(null);
const actionOpen = ref(false);
function onAction(id: string): void {
  if (id === MOVE || id === CLOSE) {
    // Once the menu has closed and given the focus back to its button: the panel then takes
    // it to its first field, and gives it back to that button when it closes.
    const mode = id === MOVE ? "move" : "close";
    void nextTick(() =>
      requestAnimationFrame(() =>
        openEdit({ mode, collection: props.collection.id, id: props.id }),
      ),
    );
    return;
  }
  acting.value = placed(pluginActions.value, id) ?? null;
  actionOpen.value = acting.value !== null;
}

// The sections of the VLAN (026): the VLAN of a group, the networks of a VLAN, the VLAN of a
// prefix (with `core.vlan.read`).
const vlanTab = computed(
  () => props.collection.kind === "prefix" && session.value.rights.has("core.vlan.read"),
);
const vlanBlocked = computed(() => blockedBy(editRight("core.vlan")));
function createVlan(): void {
  openEdit({ mode: "create", collection: "vlans", preset: { group: props.id } });
}

const tabs = computed<TabItem[]>(() => [
  { value: "properties", label: t("views.record.properties") },
  ...(hasContent.value ? [{ value: "content", label: t("views.record.content") }] : []),
  ...(props.collection.kind === "vlan_group" ? [{ value: "vlans", label: t("nav.vlans") }] : []),
  ...(props.collection.kind === "vlan"
    ? [{ value: "networks", label: t("views.vlan.networks") }]
    : []),
  ...(vlanTab.value ? [{ value: "vlan", label: t("nav.vlans") }] : []),
  { value: "history", label: t("views.record.history") },
  { value: "anomalies", label: t("views.record.anomalies") },
  ...tabItems(pluginTabs.value, tr),
]);
const tab = computed({
  get: () => {
    const asked = route.query.tab;
    return typeof asked === "string" && tabs.value.some((tab) => tab.value === asked)
      ? asked
      : "properties";
  },
  set: (next: string) => {
    const query = { ...route.query, tab: next === "properties" ? undefined : next };
    void router.replace({ query });
  },
});

/** The range of VIDs of a group of VLAN (`100–199`). */
const range = computed(() =>
  typeof item.value?.vid_min === "number" && typeof item.value.vid_max === "number"
    ? `${item.value.vid_min}–${item.value.vid_max}`
    : null,
);
/** The object a field names by its identifier only: the group of a VLAN, an anchor. */
const referenced = (field: string) =>
  field === "anchor"
    ? anchorOf(item.value?.anchor)
    : field === "group" && isSummary(item.value?.group)
      ? { kind: "vlan_group", id: item.value!.group.id }
      : null;

/** The attributes of each kind, the identifier last (copiable). */
const FIELDS: Record<string, readonly string[]> = {
  domain: ["slug"],
  entity: ["slug", "parent"],
  site: ["slug", "parent"],
  prefix: ["domain", "nature", "point_to_point", "anycast", "public"],
  range: ["domain", "first", "last", "usage", "public"],
  address: ["domain", "usage", "public"],
  vlan_group: ["slug", "anchor", "range"],
  vlan: ["vid", "name", "status", "group"],
};
const MONO = new Set(["slug", "first", "last", "parent", "domain", "range", "vid", "name"]);
const attributes = computed(() => [
  ...(FIELDS[props.collection.kind] ?? []).map((field) => {
    const value = field === "range" ? range.value : item.value?.[field];
    return {
      key: field,
      label: t(`views.field.${field}`),
      value: typeof value === "boolean" ? t(`views.value.boolean.${value}`) : fieldText(value),
      mono: MONO.has(field),
    };
  }),
  { key: "id", label: t("views.field.id") },
]);
const linkOf = (field: string) => {
  const value = item.value?.[field];
  return isSummary(value) ? pageOfHref(value.href) : null;
};
</script>

<template>
  <h1 class="k-visually-hidden">{{ kindWords }} {{ name }}</h1>
  <span class="k-visually-hidden" role="status">{{ said }}</span>
  <ProblemState
    v-if="record.isError.value"
    :title="t('views.record.error', { kind: kindWords })"
    :error="record.error.value"
    :retry="record.refetch"
  >
    {{ t("views.errorText") }}
  </ProblemState>
  <ViewPanel v-else :title="name" :detail="detail" main>
    <template #actions>
      <Button
        icon="pencil"
        :shortcut="settings.singleKeys.value ? ['e'] : undefined"
        v-bind="editBlocked"
        @click="modify"
        >{{ t("edit.modifyButton") }}</Button
      >
      <Menu
        :label="t('views.actions')"
        icon="ellipsis"
        align="end"
        :items="actions"
        @select="onAction"
      />
    </template>
    <Split side="end" side-size="sm" gap="3">
      <Skeleton v-if="record.isPending.value" variant="rows" :rows="6" />
      <Tabs v-else v-model="tab" :tabs="tabs" :label="t('views.record.sections')">
        <template #properties>
          <PropertiesTab
            :id="props.id"
            :collection="props.collection"
            :name="name"
            :version="typeof item?.version === 'string' ? item.version : null"
          />
        </template>
        <template v-if="hasContent" #content>
          <ContentTab v-if="contentKind" :id="props.id" :kind="contentKind" :name="name" />
        </template>
        <template v-if="props.collection.kind === 'vlan_group'" #vlans>
          <GroupVlansTab :id="props.id" :name="name" :blocked="vlanBlocked" @create="createVlan" />
        </template>
        <template v-if="props.collection.kind === 'vlan'" #networks>
          <VlanNetworksTab :id="props.id" :name="name" />
        </template>
        <template v-if="vlanTab" #vlan>
          <NetworkVlanTab :id="props.id" :name="name" :nature="item?.nature" />
        </template>
        <template #history>
          <HistoryTab :id="props.id" :kind="props.collection.kind" :name="name" />
        </template>
        <template #anomalies>
          <AnomaliesTab :id="props.id" :kind="props.collection.kind" :name="name" />
        </template>
        <template v-for="extension in pluginTabs" :key="placeId(extension)" #[placeId(extension)]>
          <ExtensionElement
            :tag="extension.element"
            :plugin="extension.plugin"
            :label="labelOf(tr, extension)"
            :object="object"
          />
        </template>
      </Tabs>
      <template #side>
        <Stack gap="3">
          <ViewPanel :title="t('views.record.attributes')">
            <Skeleton v-if="record.isPending.value" variant="text" :rows="4" />
            <AttributeList v-else :items="attributes">
              <template v-for="field in ['domain', 'parent']" :key="field" #[field]>
                <KLink v-if="linkOf(field)" :to="linkOf(field)!">{{
                  fieldText(item?.[field])
                }}</KLink>
                <template v-else>{{ fieldText(item?.[field]) }}</template>
              </template>
              <template #nature>
                <KeyLabel
                  :value="String(item?.nature)"
                  :label="valueWords('nature', item?.nature)"
                />
              </template>
              <template #usage>
                <KeyLabel :value="String(item?.usage)" :label="valueWords('usage', item?.usage)" />
              </template>
              <template v-for="field in ['anchor', 'group']" :key="field" #[field]>
                <ObjectName
                  v-if="referenced(field)"
                  :id="referenced(field)!.id"
                  :kind="referenced(field)!.kind"
                  :with-kind="field === 'anchor'"
                />
                <template v-else>—</template>
              </template>
              <template #status>
                <StatusBadge
                  v-if="VLAN_STATES[String(item?.status)]"
                  :state="VLAN_STATES[String(item?.status)]!"
                  :label="valueWords('status', item?.status) ?? String(item?.status)"
                />
              </template>
              <template #id><CodeBadge :code="props.id" /></template>
            </AttributeList>
          </ViewPanel>
          <OccupancyPanel v-if="props.collection.kind === 'prefix'" :id="props.id" />
          <VlanOccupancyPanel
            v-if="props.collection.kind === 'vlan_group'"
            :id="props.id"
            :name="name"
            :range="range"
          />
          <ViewPanel
            v-for="extension in pluginPanels"
            :key="placeId(extension)"
            :title="labelOf(tr, extension)"
            :detail="t('views.home.plugin', { name: extension.plugin })"
          >
            <ExtensionElement
              :tag="extension.element"
              :plugin="extension.plugin"
              :label="labelOf(tr, extension)"
              :object="object"
            />
          </ViewPanel>
        </Stack>
      </template>
    </Split>
  </ViewPanel>
  <PluginAction v-model:open="actionOpen" :extension="acting" :object="object" />
</template>
