<script setup lang="ts">
// The source of a resolved value, in the column « Source » of the properties (task
// 021-11p4, Q-021-23; maquette E2): « Directe »; « Héritée de » its origin, named and linked;
// inherited from an object the caller may not read, said without naming it (REQ-005-19);
// the default of a source of expected values, its key and revision; derived by a module,
// its motif (or the label it read) copiable — the words of the explanation
// (`ExplanationBody`). A masked value has no source (REQ-005-21): what it lacks is said.
// A value without one (a row of the draft alone) shows none.
import { CodeBadge, KeyLabel, NoValue } from "@kartenn/ui";
import { useI18n } from "vue-i18n";
import ObjectName from "./ObjectName.vue";
import type { ValueOrigin } from "./provenance.ts";

const props = defineProps<{ origin: ValueOrigin | null; masked?: boolean }>();
const { t } = useI18n();
</script>

<template>
  <span v-if="props.masked">{{ t("views.record.sourceMasked") }}</span>
  <NoValue v-else-if="props.origin === null" />
  <span v-else-if="props.origin.kind === 'direct'">{{ t("views.record.sourceDirect") }}</span>
  <span v-else-if="props.origin.kind === 'unreadable'">{{
    t("views.record.sourceUnreadable")
  }}</span>
  <span v-else-if="props.origin.kind === 'inherited'"
    >{{ `${t("views.record.sourceInherited")} `
    }}<ObjectName
      :id="props.origin.origin.id"
      :kind="props.origin.origin.kind"
      :href="props.origin.origin.href"
      with-kind
  /></span>
  <span v-else-if="props.origin.provenance.kind === 'source'"
    >{{ `${t("views.record.sourceDefault")} `
    }}<KeyLabel :value="props.origin.provenance.source" /><template
      v-if="props.origin.provenance.revision"
      >{{ ` ${t("views.anomalies.revision")} `
      }}<span class="k-mono">{{ props.origin.provenance.revision }}</span></template
    ></span
  >
  <span v-else-if="props.origin.provenance.kind === 'label'"
    >{{ `${t("views.record.fromLabel", { module: props.origin.provenance.module })} `
    }}<CodeBadge :code="props.origin.provenance.key"
  /></span>
  <span v-else-if="props.origin.provenance.kind === 'module'"
    >{{ `${t("views.record.byModule", { module: props.origin.provenance.module })} `
    }}<CodeBadge :code="props.origin.provenance.motif"
  /></span>
  <span v-else
    >{{ `${t("views.record.sourceOther")} ` }}<CodeBadge :code="props.origin.provenance.code"
  /></span>
</template>
