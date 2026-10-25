<script setup lang="ts">
// The action bar of a panel of the mock-ups (024/M15, « Structure avant style »): a state
// or a short context at the start, then a Toolbar that takes the rest of the line and folds
// its actions into « … » rather than wrapping. Not in the inventory of the 020: layout
// written for the mock-ups (task 11 of the 024), to be decided with the frame of the 021.
import { Toolbar, type ToolbarAction } from "@kartenn/ui";

const props = withDefaults(
  defineProps<{
    label: string;
    actions: ToolbarAction[];
    /** Where the actions sit: at the start when the right panel may cover the end. */
    align?: "start" | "end";
  }>(),
  { align: "end" },
);
</script>

<template>
  <div class="k-ecran-barre">
    <div v-if="$slots.default" class="k-ecran-barre__start"><slot /></div>
    <Toolbar :label="props.label" :actions="props.actions" :align="props.align" />
  </div>
</template>

<style>
@layer components {
  .k-ecran-barre {
    display: flex;
    gap: var(--k-space-3);
    align-items: center;
    min-inline-size: 0;
  }

  .k-ecran-barre__start {
    display: flex;
    flex-shrink: 0;
    gap: var(--k-space-2);
    align-items: center;
  }

  /* The toolbar takes the rest of the line and adds nothing to the min-content width of
   * the bar: an auto track around it (the panel) does not grow, the toolbar folds. */
  .k-ecran-barre > .k-toolbar {
    flex-grow: 1;
    inline-size: 0;
  }
}
</style>
