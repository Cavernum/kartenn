<script setup lang="ts">
// A write that did not go through, said where it was asked (REQ-021-16, 20; design 021
// « Édition en place »): 412, the object changed since it was read — the input kept, the
// value read again given beside it; 422, the violations with their codes; 403 (or
// `core.object.forbidden`), a refusal; another problem or the network. Then the problem
// itself, in plain words with its `type`, and the `Request-Id`, copyable (`problems/`).
// The input stays in its fields whatever the answer.
import { FORBIDDEN_RULE } from "@kartenn/sdk";
import { InlineAlert } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useProblems, type ProblemLine } from "../../problems/index.ts";
import type { Refusal } from "./inline.ts";

const props = defineProps<{
  refusal: Refusal;
  /** After a 412: the changed fields, their value now and the one typed. */
  changed?: readonly { label: string; current: string; input: string }[];
}>();
const { t } = useI18n();
const { lines } = useProblems();

/** What the refusal is about: the fields changed, the violations, the rules denied. */
const lead = computed<ProblemLine[]>(() => {
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
  return (refusal.problem?.violations ?? [])
    .filter((v) => FORBIDDEN_RULE.test(v.rule))
    .map((v) => ({ message: v.detail, code: v.rule }));
});
/** The title says the problem but for another failure: then the sentence of its type. */
const said = computed(() => props.refusal.kind !== "failed" || !props.refusal.problem);
const shown = computed(() => lines(props.refusal.problem, lead.value, { said: said.value }));
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
      return props.refusal.problem ? t("edit.failed") : t("problem.network");
  }
});
</script>

<template>
  <InlineAlert :tone="tone" :title="title" :items="shown">
    <template v-if="props.refusal.kind === 'conflict'">{{ t("edit.conflictText") }}</template>
    <template v-else>{{ t("edit.kept") }}</template>
  </InlineAlert>
</template>
