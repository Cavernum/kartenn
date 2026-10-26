<script setup lang="ts">
// The draft tab of the right panel (spec 021, « Cadre » and « Écrans » : Brouillons; 024/M8):
// the operations of the active draft, each with its remove button and, after a
// simulation, its violations in plain language with their code (024/M16). The frame shows
// it in every mock-up; E5 shows its states. The footer (simulation, motive, action) is
// `EcranSoumission`.
import { Cluster, DiffValue, Icon, IconButton, InlineAlert, Stack } from "@kartenn/ui";
import { computed } from "vue";
import { DRAFT_OPERATIONS } from "./donnees.ts";

const props = withDefaults(
  defineProps<{
    /** How many operations the draft holds (the first ones of the fixture). */
    count: number;
    /** The violations of the last simulation, by object key. */
    violations?: Record<string, { message: string; code: string }>;
  }>(),
  { violations: () => ({}) },
);
const operations = computed(() => DRAFT_OPERATIONS.slice(0, props.count));
</script>

<template>
  <Stack gap="3">
    <p v-if="!operations.length" class="k-ecran-muted">
      Le brouillon est vide : une modification en place ou un ajout l'y range.
    </p>
    <Stack v-else as="ol" gap="2" class="k-ecran-list" aria-label="Opérations du brouillon">
      <li v-for="op in operations" :key="op.key">
        <Stack gap="1">
          <Cluster justify="between" :wrap="false" gap="2">
            <Cluster gap="2" :wrap="false" align="baseline" class="k-ecran-op">
              <Icon name="draft" />
              <code>{{ op.key }}</code>
              <span class="k-ecran-op__change">
                {{ op.what }} :
                <DiffValue v-if="op.before" kind="changed">
                  <template #before>{{ op.before }}</template>
                  {{ op.after }}
                </DiffValue>
                <DiffValue v-else kind="added">{{ op.after }}</DiffValue>
              </span>
            </Cluster>
            <IconButton
              icon="x"
              size="sm"
              :label="`Retirer l'opération « ${op.what} » sur ${op.key}`"
            />
          </Cluster>
          <InlineAlert
            v-if="props.violations[op.key]"
            tone="ko"
            :title="`${op.key} : refusée par la simulation`"
            :items="[props.violations[op.key]!]"
          />
        </Stack>
      </li>
    </Stack>
  </Stack>
</template>

<style>
@layer components {
  /* An operation: the key and the button stay whole, the change wraps beside the key,
   * never cut (revue du 2026-10-25). */
  .k-ecran-op {
    min-inline-size: 0;
  }

  .k-ecran-op > .k-icon,
  .k-ecran-op > code {
    flex: none;
  }

  .k-ecran-op__change {
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }
}
</style>
