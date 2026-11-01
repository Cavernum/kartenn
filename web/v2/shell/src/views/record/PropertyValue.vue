<script setup lang="ts">
// The value of a resolved property (003): a text, a number, a date, a key of a closed list
// (its label, the key copiable), a reference to an object (its name, a link), an external
// reference (`<system>:<identifier>`), several of them for a multiple property.
import { KeyLabel } from "@kartenn/ui";
import { computed } from "vue";
import { useKeyLabel } from "../data.ts";
import { isSummary } from "../collections.ts";
import ObjectName from "./ObjectName.vue";

const props = defineProps<{
  property: string;
  value: unknown;
  /** The declared type (`enum`, `ref`…) and, for a reference, the kind it targets. */
  type?: string | undefined;
  target?: string | undefined;
}>();
const keyLabel = useKeyLabel();
const values = computed<unknown[]>(() =>
  Array.isArray(props.value) ? props.value : props.value === null ? [] : [props.value],
);
const external = (value: unknown) => {
  const v = value as { system?: unknown; id?: unknown };
  return typeof v.system === "string" && typeof v.id === "string" ? `${v.system}:${v.id}` : null;
};
</script>

<template>
  <span v-if="values.length === 0">—</span>
  <span v-else>
    <template v-for="(one, i) in values" :key="i">
      <template v-if="i > 0">, </template>
      <ObjectName
        v-if="isSummary(one)"
        :id="one.id"
        :kind="(one as { kind?: string | null }).kind"
        :href="one.href"
      />
      <ObjectName
        v-else-if="props.type === 'ref' && typeof one === 'string' && props.target"
        :id="one"
        :kind="props.target"
      />
      <span v-else-if="external(one)" class="k-mono">{{ external(one) }}</span>
      <KeyLabel
        v-else-if="props.type === 'enum' && typeof one === 'string'"
        :value="one"
        :label="keyLabel('value', `${props.property}/${one}`)"
        plain
      />
      <span v-else-if="typeof one === 'object'" class="k-mono">{{ JSON.stringify(one) }}</span>
      <span v-else :class="{ 'k-num': typeof one === 'number' }">{{ String(one) }}</span>
    </template>
  </span>
</template>
