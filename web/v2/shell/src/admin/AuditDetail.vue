<script setup lang="ts">
// The event of the audit journal chosen, in the Détail tab of the right panel (012/REQ-
// 012-17; REQ-021-12): found again among the pages of the journal the cache holds (the API
// serves no event alone); its name and outcome at its top, then in aligned lines what
// happened (instant, severity, criticality, reason, motive), who did it (kind, identity,
// authentication, issuer, source), the objects it names, its correlation (changeset,
// request, trace, instance) and its details as journaled. An event of a page not read says
// so, the filters of the list to find it.
import { keys } from "@kartenn/sdk";
import { EmptyState, Skeleton, Stack, StatusBadge } from "@kartenn/ui";
import { computed, onScopeDispose, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import { useQueryClient } from "../data/query.ts";
import { useSession } from "../rights/index.ts";
import { keepRecordName } from "../views/names.ts";
import { holdsGlobally } from "./access.ts";
import AdminLines from "./AdminLines.vue";
import { cachedEvent, outcomeBadge, type AuditEvent } from "./audit.ts";
import { useAuditWords } from "./auditWords.ts";

const props = defineProps<{ id: string }>();
const { t } = useI18n();
const session = useSession();
const queryClient = useQueryClient();
const words = useAuditWords(() => holdsGlobally(session.value.me, "core.rbac.read"));

const JOURNAL = keys.allOn("/v1/audit-events");
const event = shallowRef<AuditEvent | undefined>(undefined);
const reading = shallowRef(false);
const look = () => {
  const found = cachedEvent(queryClient, props.id);
  if (found !== event.value) event.value = found;
  reading.value = queryClient.isFetching({ queryKey: JOURNAL }) > 0;
};
watch(() => props.id, look, { immediate: true });
// The pages arrive, or are read again: the event looked for in them anew.
onScopeDispose(
  queryClient.getQueryCache().subscribe((change) => {
    const key = change.query.queryKey;
    if (key[0] === JOURNAL[0] && key[1] === JOURNAL[1]) look();
  }),
);
watch(
  () => event.value?.name,
  (name) => name && keepRecordName(props.id, name),
  { immediate: true },
);

/** The identity of the actor, by its name when read, with its page when it has one. */
const actorPage = computed(() => {
  const e = event.value;
  return e?.actor_id && (e.actor_kind === "person" || e.actor_kind === "service_account")
    ? `/admin/identities/${encodeURIComponent(e.actor_id)}`
    : null;
});
interface Line {
  key: string;
  label: string;
  value: string | null | undefined;
  mono?: boolean;
}
const shown = (lines: Line[]) => lines.filter((line) => line.value);
const happened = computed<Line[]>(() => {
  const e = event.value;
  if (!e) return [];
  return shown([
    { key: "occurred", label: t("admin.audit.field.occurred"), value: words.when(e.occurred_at) },
    {
      key: "severity",
      label: t("admin.audit.field.severity"),
      value: words.severity(e.severity),
      mono: !words.severityKnown(e.severity),
    },
    {
      key: "criticality",
      label: t("admin.audit.field.criticality"),
      value: words.criticality(e.criticality),
    },
    { key: "reason", label: t("admin.audit.field.reason"), value: e.reason },
    { key: "motive", label: t("admin.audit.field.motive"), value: e.motive },
  ]);
});
const actor = computed<Line[]>(() => {
  const e = event.value;
  if (!e) return [];
  return shown([
    { key: "kind", label: t("admin.audit.field.kind"), value: words.kind(e.actor_kind) },
    { key: "auth", label: t("admin.audit.field.authMethod"), value: e.auth_method, mono: true },
    { key: "issuer", label: t("admin.audit.field.issuer"), value: e.issuer, mono: true },
    { key: "claimed", label: t("admin.audit.field.claimed"), value: e.claimed, mono: true },
    { key: "ip", label: t("admin.audit.field.sourceIp"), value: e.source_ip, mono: true },
    { key: "agent", label: t("admin.audit.field.userAgent"), value: e.user_agent, mono: true },
  ]);
});
const correlation = computed<Line[]>(() => {
  const e = event.value;
  if (!e) return [];
  return shown([
    { key: "request", label: t("admin.audit.field.request"), value: e.request_id, mono: true },
    { key: "trace", label: t("admin.audit.field.trace"), value: e.trace_id, mono: true },
    { key: "instance", label: t("admin.audit.field.instance"), value: e.instance_id, mono: true },
    {
      key: "version",
      label: t("admin.audit.field.version"),
      value: e.service_version,
      mono: true,
    },
  ]);
});
/** The objects named, the first ones only: the whole list is in the export. */
const TARGETS_SHOWN = 20;
const targets = computed(() => {
  const ids = event.value?.target_ids ?? [];
  return { shown: ids.slice(0, TARGETS_SHOWN), more: Math.max(0, ids.length - TARGETS_SHOWN) };
});
const details = computed(() =>
  event.value?.details && Object.keys(event.value.details).length > 0
    ? JSON.stringify(event.value.details, null, 2)
    : null,
);
</script>

<template>
  <Skeleton v-if="!event && reading" variant="rows" :rows="8" />
  <EmptyState v-else-if="!event" :title="t('admin.audit.unread')">
    {{ t("admin.audit.unreadText") }}
  </EmptyState>
  <Stack v-else gap="4" class="k-admin-detail">
    <Stack gap="2">
      <h2 class="k-admin-heading k-mono">{{ event.name }}</h2>
      <span
        ><StatusBadge :state="outcomeBadge(event.outcome)" :label="words.outcome(event.outcome)"
      /></span>
    </Stack>

    <section>
      <Stack gap="2">
        <h3 class="k-admin-heading">
          {{ t("admin.audit.section.event") }}
        </h3>
        <AdminLines :label="t('admin.audit.section.event')" read class="k-admin-facts">
          <li v-for="line in happened" :key="line.key">
            <span>{{ line.label }}</span>
            <span class="k-admin-lines__value" :class="{ 'k-mono': line.mono }">{{
              line.value
            }}</span>
          </li>
        </AdminLines>
      </Stack>
    </section>

    <section>
      <Stack gap="2">
        <h3 class="k-admin-heading">
          {{ t("admin.audit.section.actor") }}
        </h3>
        <AdminLines :label="t('admin.audit.section.actor')" read class="k-admin-facts">
          <li v-if="event.actor_id">
            <span>{{ t("admin.audit.field.identity") }}</span>
            <span class="k-admin-lines__value">
              <RouterLink
                v-if="actorPage"
                :to="actorPage"
                :class="{ 'k-mono': !words.identity(event.actor_id) }"
                >{{ words.identity(event.actor_id) ?? event.actor_id }}</RouterLink
              >
              <span v-else class="k-mono">{{ event.actor_id }}</span>
            </span>
          </li>
          <li v-for="line in actor" :key="line.key">
            <span>{{ line.label }}</span>
            <span class="k-admin-lines__value" :class="{ 'k-mono': line.mono }">{{
              line.value
            }}</span>
          </li>
        </AdminLines>
      </Stack>
    </section>

    <section v-if="event.target_kind">
      <Stack gap="2">
        <h3 class="k-admin-heading">
          {{ t("admin.audit.section.targets", { n: event.target_ids?.length ?? 0 }) }}
          <span class="k-admin-muted k-mono">{{ event.target_kind }}</span>
        </h3>
        <AdminLines :label="t('admin.audit.section.targetList')" read>
          <li v-for="target in targets.shown" :key="target">
            <span class="k-mono">{{ target }}</span>
          </li>
        </AdminLines>
        <p v-if="targets.more > 0" class="k-admin-muted">
          {{ t("admin.audit.moreTargets", { n: targets.more }, targets.more) }}
        </p>
      </Stack>
    </section>

    <section>
      <Stack gap="2">
        <h3 class="k-admin-heading">
          {{ t("admin.audit.section.correlation") }}
        </h3>
        <AdminLines :label="t('admin.audit.section.correlation')" read class="k-admin-facts">
          <li v-if="event.changeset_id">
            <span>{{ t("admin.audit.field.changeset") }}</span>
            <span class="k-admin-lines__value"
              ><RouterLink
                class="k-mono"
                :to="`/changesets/${encodeURIComponent(event.changeset_id)}`"
                >{{ event.changeset_id }}</RouterLink
              ></span
            >
          </li>
          <li v-for="line in correlation" :key="line.key">
            <span>{{ line.label }}</span>
            <span class="k-admin-lines__value" :class="{ 'k-mono': line.mono }">{{
              line.value
            }}</span>
          </li>
        </AdminLines>
      </Stack>
    </section>

    <section v-if="details">
      <Stack gap="2">
        <h3 class="k-admin-heading">
          {{ t("admin.audit.section.details") }}
        </h3>
        <pre
          class="k-admin-json k-mono"
          tabindex="0"
          role="region"
          :aria-label="t('admin.audit.section.details')"
          >{{ details }}</pre>
      </Stack>
    </section>
  </Stack>
</template>
