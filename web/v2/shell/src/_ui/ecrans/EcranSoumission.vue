<script setup lang="ts">
// The footer of the draft tab (spec 021, « Cadre » : pied de l'onglet Brouillon; 024/M9):
// the result of the simulation (its sentence wraps, never cut), the motive in two fields (CommitForm), the
// split button. Its default action follows the rights and the simulation: « Appliquer »
// for who holds `apply_own` and when no policy holds the changeset, else « Soumettre en
// revue »; an impossible action stays in the menu, disabled, with its reason.
// « Appliquer » prefills a generic summary.
import { Cluster, CommitForm, Icon, SplitButton, StatusBadge, type SplitAction } from "@kartenn/ui";
import { computed, ref } from "vue";

const props = withDefaults(
  defineProps<{
    /** The simulation: under way, with violations, valid. */
    simulation?: "running" | "violations" | "valid";
    /** A policy holds the changeset: « Appliquer » is refused, the review is the default. */
    held?: boolean;
    /** The name of the policy that holds it. */
    policy?: string;
    /** The chevron of the split button open (its menu in the layer of the frame). */
    menuOpen?: boolean;
    summary?: string;
    description?: string;
  }>(),
  {
    simulation: "valid",
    policy: "Revue des VIP",
    summary: "Maintenance du référentiel",
    description: "",
  },
);

const summary = ref(props.summary);
const description = ref(props.description);

const actions = computed<SplitAction[]>(() => {
  const blocked =
    props.simulation === "violations"
      ? "La simulation signale 2 violations : les corriger ou retirer leurs opérations."
      : props.simulation === "running"
        ? "La simulation n'est pas finie."
        : undefined;
  const held = props.held
    ? `La politique « ${props.policy} » retient ce changeset : il passe en revue.`
    : undefined;
  return [
    {
      id: "apply",
      label: "Appliquer",
      icon: "circle-check",
      ...(blocked || held ? { disabled: true, reason: (blocked ?? held)! } : {}),
    },
    {
      id: "review",
      label: "Soumettre en revue",
      icon: "send",
      ...(blocked ? { disabled: true, reason: blocked } : {}),
    },
    { id: "save", label: "Enregistrer le brouillon", icon: "draft" },
  ];
});
const action = ref(props.simulation === "violations" ? "save" : props.held ? "review" : "apply");
</script>

<template>
  <p class="k-ecran-simulation" role="status">
    <template v-if="props.simulation === 'running'">
      <Icon name="loader-circle" class="k-spinner" />
      <span>Simulation en cours…</span>
    </template>
    <template v-else-if="props.simulation === 'violations'">
      <StatusBadge state="ko" label="2 violations" />
      <span
        >Corriger ou retirer les opérations en cause pour appliquer ou soumettre ; le brouillon
        s'enregistre tel quel, violations comprises.</span
      >
    </template>
    <template v-else-if="props.held">
      <StatusBadge state="review" label="Revue requise" />
      <span>Simulation valide ; politique « {{ props.policy }} ».</span>
    </template>
    <template v-else>
      <StatusBadge state="ok" label="Simulation valide" />
      <span>0 violation · effet au présent</span>
    </template>
  </p>
  <CommitForm v-model:summary="summary" v-model:description="description" required collapsible />
  <Cluster justify="end">
    <SplitButton
      v-model:action="action"
      :actions="actions"
      :shortcut="['Control', 'Enter']"
      :default-open="props.menuOpen"
    />
  </Cluster>
</template>

<style>
@layer components {
  /* The result of the simulation: the badge, then the sentence, which wraps under itself
   * when the panel is narrow, never cut (revue du 2026-10-25). */
  .k-ecran-simulation {
    display: flex;
    gap: var(--k-space-2);
    align-items: baseline;
    min-inline-size: 0;
  }

  .k-ecran-simulation > .k-status-badge,
  .k-ecran-simulation > .k-icon {
    flex: none;
    align-self: center;
  }

  .k-ecran-simulation > span:last-child {
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }
}
</style>
