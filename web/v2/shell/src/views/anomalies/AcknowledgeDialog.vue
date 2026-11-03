<script setup lang="ts">
// The acknowledgement of an anomaly (REQ-021-9, REQ-002-36): a motive, required, then
// `POST /v1/anomalies/{id}/acknowledgement`; the lists of anomalies and the counters of the
// home page are read again. A refusal is said in the dialog with its codes (a dialog says
// its outcomes in itself, never by a toast); success closes it and says so by a toast.
import { api, queryClient } from "@kartenn/sdk";
import { Button, Dialog, TextField, injectToasts } from "@kartenn/ui";
import { nextTick, ref, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useMutation } from "../../data/query.ts";
import ProblemState from "../../problems/ProblemState.vue";

const props = defineProps<{ id: string | null; rule: string }>();
const open = defineModel<boolean>("open", { default: false });
/** Acknowledged: its row leaves the list, the host gives the focus a place. */
const emit = defineEmits<{ done: [] }>();
const { t } = useI18n();
const toasts = injectToasts();

const motive = ref("");
const missing = ref<string | undefined>();
watch(open, (now) => {
  if (!now) return;
  motive.value = "";
  missing.value = undefined;
  acknowledge.reset();
});

const acknowledge = useMutation({
  mutationFn: (id: string) =>
    api.post("/v1/anomalies/{id}/acknowledgement", {
      path: { id },
      body: { motive: motive.value.trim() },
    }),
  // Not awaited: the dialog closes and says so first, the lists then read again without the
  // anomaly (the grid places the focus as they come, `AnomalyGrid`).
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ["v1", "/v1/anomalies"] });
    void queryClient.invalidateQueries({ queryKey: ["v1", "/v1/stats"] });
  },
});
const form = useTemplateRef<HTMLFormElement>("form");
/** The motive refused: the focus back on it, its error said there. */
const toMotive = () =>
  void nextTick(() => form.value?.querySelector<HTMLInputElement>("input")?.focus());

async function submit(): Promise<void> {
  if (!props.id || acknowledge.isPending.value) return;
  if (motive.value.trim() === "") {
    missing.value = t("views.anomalies.motiveMissing");
    toMotive();
    return;
  }
  missing.value = undefined;
  try {
    await acknowledge.mutateAsync(props.id);
  } catch {
    return;
  }
  open.value = false;
  emit("done");
  toasts?.show({ tone: "ok", title: t("views.anomalies.acknowledged", { rule: props.rule }) });
}
</script>

<template>
  <Dialog v-model:open="open" :title="t('views.anomalies.acknowledgeTitle')" size="md">
    <form ref="form" class="k-acknowledge" novalidate @submit.prevent="submit">
      <p>{{ t("views.anomalies.acknowledgeText", { rule: props.rule }) }}</p>
      <TextField v-model="motive" :label="t('views.anomalies.motive')" :error="missing" required />
      <ProblemState
        v-if="acknowledge.isError.value"
        :title="t('views.anomalies.acknowledgeError')"
        :error="acknowledge.error.value"
      />
    </form>
    <template #actions>
      <Button variant="ghost" @click="open = false">{{ t("views.cancel") }}</Button>
      <Button variant="primary" :loading="acknowledge.isPending.value" @click="submit">{{
        t("views.anomalies.acknowledge")
      }}</Button>
    </template>
  </Dialog>
</template>

<style>
@layer components {
  .k-acknowledge {
    display: grid;
    gap: var(--k-space-3);
  }
}
</style>
