<script setup lang="ts">
// A write that did not go through, said where it was asked (REQ-021-16; design 021
// « Édition en place »): 412, the object changed since it was read — the input kept, the
// value read again given beside it; 422, the violations with their codes; 403 (or
// `core.object.forbidden`), a refusal; another problem or the network. The input stays in
// its fields whatever the answer.
import { ApiProblem, FORBIDDEN_RULE } from "@kartenn/sdk";
import { InlineAlert } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { Refusal } from "./inline.ts";

const props = defineProps<{
  refusal: Refusal;
  /** After a 412: the changed fields, their value now and the one typed. */
  changed?: readonly { label: string; current: string; input: string }[];
}>();
const { t } = useI18n();

const problem = computed(() =>
  props.refusal.problem instanceof ApiProblem ? props.refusal.problem : null,
);
const items = computed(() => {
  const refusal = props.refusal;
  if (refusal.kind === "conflict") {
    return (props.changed ?? []).map((c) => ({
      message: t("edit.conflictField", { field: c.label, current: c.current, input: c.input }),
    }));
  }
  if (refusal.kind === "invalid") {
    return [...Object.values(refusal.byField).flat(), ...refusal.general].map((v) => ({
      message: v.detail,
      code: v.rule,
    }));
  }
  const p = problem.value;
  if (!p) return [];
  const denied = p.violations.filter((v) => FORBIDDEN_RULE.test(v.rule));
  return denied.length > 0
    ? denied.map((v) => ({ message: v.detail, code: v.rule }))
    : [{ message: p.detail ?? p.title, code: p.code }];
});
/** The codes, then the `Request-Id` of the answer, copyable (020: an error gives both). */
const shown = computed(() => {
  const id = problem.value?.requestId;
  return id ? [...items.value, { message: t("edit.requestId"), code: id }] : items.value;
});
const tone = computed(() => (props.refusal.kind === "conflict" ? "warn" : "ko"));
const title = computed(() => {
  switch (props.refusal.kind) {
    case "conflict":
      return t("edit.conflict");
    case "invalid":
      return t("edit.invalid");
    case "forbidden":
      return t("edit.forbidden");
    default:
      return problem.value ? t("edit.failed") : t("edit.network");
  }
});
</script>

<template>
  <InlineAlert :tone="tone" :title="title" :items="shown">
    <template v-if="props.refusal.kind === 'conflict'">{{ t("edit.conflictText") }}</template>
    <template v-else>{{ t("edit.kept") }}</template>
  </InlineAlert>
</template>
