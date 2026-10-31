<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// What a pastille that copies its key (`KeyLabel`, `EnvChip`; REQ-020-21) puts beside its
// button: the description the button points to (`describedId`: « Clé : prod. » when a
// label hides the key, and how to copy it), the `status` region that says the outcome of a
// copy, and the bubble (decorative: the description and the region say the same) with the
// label when the button cuts it, the key, and the outcome or the hint. The key in the
// bubble is what a refused copy selects (`keyNode`).
import { computed, ref } from "vue";
import { fill, t } from "../../i18n.ts";
import Tooltip from "../Tooltip/Tooltip.vue";
import type { CopyControl } from "./copy.ts";

const props = defineProps<{
  value: string;
  label?: string | null | undefined;
  copy: CopyControl;
  describedId: string;
  /** The label is cut by the width of the button: whole in the bubble. */
  cut?: () => boolean;
}>();

const key = ref<HTMLElement | null>(null);
defineExpose({ keyNode: () => key.value });

const said = computed(() =>
  props.copy.outcome.value === "copied"
    ? t("keyCopied")
    : props.copy.outcome.value === "failed"
      ? t("copyByHand")
      : "",
);
const description = computed(() =>
  props.label ? `${fill("keyOf", { key: props.value })}. ${t("keyCopyHint")}` : t("keyCopyHint"),
);
</script>

<template>
  <span :id="props.describedId" hidden>{{ description }}</span>
  <span role="status" class="k-visually-hidden">{{ said }}</span>
  <Tooltip v-if="props.copy.tip.open.value" :control="props.copy.tip" decorative>
    <span v-if="props.label && props.cut?.()" class="k-tooltip__detail">{{ props.label }}</span>
    <span class="k-tooltip__name"
      ><span
        >{{ t("filterKey") }} <code ref="key">{{ props.value }}</code></span
      ></span
    >
    <span class="k-tooltip__detail">{{ said || t("keyCopyHint") }}</span>
  </Tooltip>
</template>
