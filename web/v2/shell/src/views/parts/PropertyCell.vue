<script setup lang="ts">
// A cell of a column of a property in a list (task 12i, Q-021-18): the value resolved for
// the object of its row, as the record's tab shows it (`PropertyValue`); none, a value
// absent (`NoValue`); a value the reader may not read, said masked; until it is read, an
// ellipsis; a read refused, said short with the code of its problem (`problems/`), as the
// VLAN column does. Each sign is said in words to assistive technologies; `note`, what an
// edit of the cell writes (a value over an inherited one), is read with the cell.
import { CodeBadge, NoValue } from "@kartenn/ui";
import { useI18n } from "vue-i18n";
import { problemCode } from "../../problems/index.ts";
import type { PropertyCellState } from "../propertyColumns.ts";
import PropertyValue from "../record/PropertyValue.vue";

const props = defineProps<{
  property: string;
  state: PropertyCellState;
  /** The declared type (`enum`, `ref`…) and, for a reference, the kind it targets. */
  type?: string | undefined;
  target?: string | undefined;
  note?: string | undefined;
}>();
const { t } = useI18n();
</script>

<template>
  <span v-if="props.state.kind === 'failed'"
    >{{ t("views.list.propertyUnreadable") }}
    <CodeBadge v-if="problemCode(props.state.error)" :code="problemCode(props.state.error)!"
  /></span>
  <span v-else-if="props.state.kind === 'reading'"
    ><span aria-hidden="true">…</span
    ><span class="k-visually-hidden">{{ t("views.list.propertyReading") }}</span></span
  >
  <span v-else-if="props.state.kind === 'masked'">{{ t("views.record.masked") }}</span>
  <NoValue v-else-if="props.state.value === null" />
  <PropertyValue
    v-else
    :property="props.property"
    :value="props.state.value"
    :type="props.type"
    :target="props.target"
  />
  <span v-if="props.note" class="k-visually-hidden">{{ props.note }}</span>
</template>
