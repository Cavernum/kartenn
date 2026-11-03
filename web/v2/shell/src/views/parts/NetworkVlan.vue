<script setup lang="ts">
// The VLAN a network carries, in a cell of the list of prefixes (REQ-026-6; maquette E12,
// colonne VLAN : la propriété `core.vlan` d'un réseau): read with the properties of its
// row at the dates of the page, only while the cell is in view and a few at a time
// (`limited`, the name of the VLAN too), the read under way given up when it leaves the
// view; its name a link to its record. A container carries none (« — » without a read); a
// property the reader may not read is said masked, never « no VLAN ». Until it is read, an
// ellipsis; a read refused, said short with the code of its problem (`problems/`). Each
// sign is said in words to assistive technologies.
import { queryClient, reads } from "@kartenn/sdk";
import { CodeBadge, NoValue } from "@kartenn/ui";
import { computed, onBeforeUnmount, onMounted, ref, useTemplateRef } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import { problemCode } from "../../problems/index.ts";
import { useTime } from "../../time/index.ts";
import ObjectName from "../record/ObjectName.vue";
import { limited, vlanOfProperties, type Resolved } from "./vlan.ts";

const props = defineProps<{ id: string; network: boolean }>();
const time = useTime();
const { t } = useI18n();

// In view: the grid mounts every row up to 200, the cell reads only once it is seen.
const root = useTemplateRef<HTMLElement>("root");
const inView = ref(false);
let observer: IntersectionObserver | undefined;
onMounted(() => {
  if (typeof IntersectionObserver === "undefined" || !root.value) {
    inView.value = true;
    return;
  }
  observer = new IntersectionObserver((entries) => {
    inView.value = entries.some((entry) => entry.isIntersecting);
    // Scrolled away before its answer: the read is given up (sent again once seen).
    if (!inView.value && resolved.isFetching.value) {
      void queryClient.cancelQueries({ queryKey: options().queryKey, exact: true });
    }
  });
  observer.observe(root.value);
});
onBeforeUnmount(() => observer?.disconnect());

const options = () =>
  reads.sub<Resolved>("prefix", props.id, "properties", null, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  });
const resolved = useQuery(() => {
  const read = options();
  return {
    ...read,
    queryFn: (context: Parameters<typeof read.queryFn>[0]) =>
      limited(context.signal, () => read.queryFn(context)),
    enabled: props.network && inView.value,
  };
});
const vlan = computed(() =>
  resolved.data.value ? vlanOfProperties(resolved.data.value.data) : null,
);
</script>

<template>
  <span ref="root">
    <NoValue v-if="!props.network" :label="t('views.vlan.notNetworkCell')" />
    <!-- Short in a row (the record's tab says the whole problem); its code copiable. -->
    <span v-else-if="resolved.isError.value"
      >{{ t("views.vlan.unreadable") }}
      <CodeBadge
        v-if="problemCode(resolved.error.value)"
        :code="problemCode(resolved.error.value)!"
    /></span>
    <span v-else-if="vlan === null"
      ><span aria-hidden="true">…</span
      ><span class="k-visually-hidden">{{ t("views.vlan.reading") }}</span></span
    >
    <span v-else-if="vlan.kind === 'masked'">{{ t("views.record.masked") }}</span>
    <ObjectName v-else-if="vlan.kind === 'vlan'" :id="vlan.id" kind="vlan" capped />
    <NoValue v-else :label="t('views.vlan.noVlanCell')" />
  </span>
</template>
