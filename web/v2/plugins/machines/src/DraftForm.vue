<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A form of the plugin in Vue (task 18cb): its submission adds one operation of the module
// to the active draft of the tab (`draft.add`, W10), never a changeset of its own — the
// draft is reviewed, simulated and submitted by the shell, with its motive. The fields are
// its slot; `operation` checks them (their errors are the caller's) and gives the operation,
// or `null` when one is refused: the first field said invalid then takes the focus. A
// refusal of the server is an alert with its codes, its violations given to `violations`
// for their fields. Without the permission, the button stays, refused with its reason.
// The caller offers it at the present only (W9).
import { ApiProblem, draft, type Operation } from "@kartenn/sdk";
import { Button, Cluster, InlineAlert, Stack } from "@kartenn/ui";
import { computed, nextTick, ref, shallowRef, useTemplateRef } from "vue";
import { useHosted, usePresent, useTexts } from "./host.ts";
import { linesOf } from "./problems.ts";

const props = defineProps<{
  /** What the operation asks of the user. */
  permission: string;
  /** The operation of the fields, or `null` when a field is refused (nothing sent). */
  operation: () => Operation | null;
  /** The violations of a refusal of the server, said on their fields; a field said refused. */
  violations?: ((problem: ApiProblem) => boolean) | undefined;
}>();
const emit = defineEmits<{
  /** The operation is in the draft: the caller clears its fields. */
  added: [];
}>();

const t = useTexts();
const { context } = useHosted();
const present = usePresent();
const form = useTemplateRef<HTMLFormElement>("form");

const allowed = computed(() => context.value.user.has(props.permission));
const adding = ref(false);
/** The outcome of the last addition, gone once the form is changed again. */
const said = ref("");
const refusal = shallowRef<unknown>(null);

/** The first field said refused takes the focus, once drawn. */
async function focusRefused(): Promise<void> {
  await nextTick();
  form.value?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus();
}

async function submit(): Promise<void> {
  if (!present() || adding.value) return;
  if (!allowed.value) {
    // Enter in a field submits too: the refusal said, as the button's tooltip says it.
    // Emptied first: the same sentence twice is said twice.
    said.value = "";
    await nextTick();
    said.value = `${t("form.needs")} ${props.permission}`;
    return;
  }
  said.value = "";
  refusal.value = null;
  const operation = props.operation();
  if (!operation) return focusRefused();
  adding.value = true;
  // Said while it is sent: `aria-busy` on a button is seldom read.
  said.value = t("form.adding");
  try {
    const outcome = await draft.add(operation);
    if (outcome.kind === "saved") {
      said.value = t("form.added", { n: outcome.draft.operation_count });
      emit("added");
      return;
    }
    refused(outcome.problem);
  } catch (error) {
    refused(error);
  } finally {
    adding.value = false;
  }
}

/** The refusal as an alert, each violation on its field too, the first one focused. */
function refused(problem: unknown): void {
  said.value = "";
  refusal.value = problem;
  if (problem instanceof ApiProblem && props.violations?.(problem)) void focusRefused();
}

/** What the last addition said no longer holds once a field changes. */
function changed(): void {
  if (adding.value) return;
  said.value = "";
  refusal.value = null;
}
</script>

<template>
  <form ref="form" novalidate @submit.prevent="submit" @input="changed" @change="changed">
    <Stack gap="4">
      <slot />
      <InlineAlert
        v-if="refusal !== null"
        tone="ko"
        :title="t('form.refused')"
        :items="linesOf(refusal, t)"
      />
      <Cluster gap="2">
        <Button
          variant="primary"
          type="submit"
          :loading="adding"
          :forbidden="allowed ? undefined : props.permission"
          >{{ t("form.submit") }}</Button
        >
        <!-- There from the start, so that what it holds is said. -->
        <span role="status">{{ said }}</span>
      </Cluster>
    </Stack>
  </form>
</template>
