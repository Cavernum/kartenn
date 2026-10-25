<script setup lang="ts">
// The place of a component not written yet (P4 of the 020): a dashed frame that names it
// and its task, around the closest stand-in. Never in a screen of the 021: the mock-ups
// only, so that the reviewer sees what is missing.
const props = defineProps<{
  component: string;
  task: string;
  /** On one line, the name after the stand-in (a control among others in a toolbar). */
  inline?: boolean;
}>();
</script>

<template>
  <div
    class="k-ecran-later"
    :data-component="props.component"
    :data-inline="props.inline ? 'true' : undefined"
  >
    <slot v-if="props.inline" />
    <p class="k-ecran-later__name">
      À venir : <code>{{ props.component }}</code> (tâche {{ props.task }})
    </p>
    <slot v-if="!props.inline" />
  </div>
</template>

<style>
@layer components {
  .k-ecran-later {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-2);
    align-content: start;
    padding: var(--k-space-2);
    border: var(--k-border-width) dashed var(--k-border-control);
    border-radius: var(--k-radius-md);
  }

  .k-ecran-later[data-inline] {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    padding: 0 var(--k-space-2) 0 0;
  }

  .k-ecran-later__name {
    min-inline-size: 0;
    overflow-wrap: anywhere;
    color: var(--k-text-muted);
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
  }
}
</style>
