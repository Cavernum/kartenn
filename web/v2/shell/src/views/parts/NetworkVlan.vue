<script setup lang="ts">
// The VLAN a network carries, in a cell of the list of prefixes (REQ-026-6; maquette E12,
// colonne VLAN : la propriété `core.vlan` d'un réseau): read with the properties of its
// row at the dates of the page, a few at a time (`limited`), its name a link to its
// record; a container carries none (« — » without a read). Until it is read, an ellipsis;
// a read refused, said short with the code of its problem (`problems/`). Each sign is said
// in words to assistive technologies.
import { reads } from "@kartenn/sdk";
import { CodeBadge } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import { problemCode } from "../../problems/index.ts";
import { useTime } from "../../time/index.ts";
import ObjectName from "../record/ObjectName.vue";
import { limited, vlanOfProperties, type Resolved } from "./vlan.ts";

const props = defineProps<{ id: string; network: boolean }>();
const time = useTime();
const { t } = useI18n();

const resolved = useQuery(() => {
  const options = reads.sub<Resolved>("prefix", props.id, "properties", null, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  });
  return {
    ...options,
    queryFn: (context: Parameters<typeof options.queryFn>[0]) =>
      limited(context.signal, () => options.queryFn(context)),
    enabled: props.network,
  };
});
const vlan = computed(() => vlanOfProperties(resolved.data.value?.data));
</script>

<template>
  <span v-if="!props.network"
    ><span aria-hidden="true">—</span
    ><span class="k-visually-hidden">{{ t("views.vlan.notNetworkCell") }}</span></span
  >
  <!-- Short in a row (the record's tab says the whole problem); its code copiable. -->
  <span v-else-if="resolved.isError.value"
    >{{ t("views.vlan.unreadable") }}
    <CodeBadge v-if="problemCode(resolved.error.value)" :code="problemCode(resolved.error.value)!"
  /></span>
  <span v-else-if="resolved.isPending.value"
    ><span aria-hidden="true">…</span
    ><span class="k-visually-hidden">{{ t("views.vlan.reading") }}</span></span
  >
  <ObjectName v-else-if="vlan" :id="vlan" kind="vlan" />
  <span v-else
    ><span aria-hidden="true">—</span
    ><span class="k-visually-hidden">{{ t("views.vlan.noVlanCell") }}</span></span
  >
</template>
