<script setup lang="ts">
import {
  DialogClose,
  DialogContent,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";
import { useI18n } from "vue-i18n";
import type { SecretShown } from "./admin.ts";

// A secret of a service account, shown once (REQ-007-19, design « Écrans et routes du
// client »): in a dialog, never kept by the shell; closing it forgets it.

defineProps<{ secret: SecretShown | null }>();
const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();
</script>

<template>
  <DialogRoot :open="secret !== null" @update:open="(open) => !open && emit('close')">
    <DialogPortal>
      <DialogOverlay class="k-overlay" />
      <DialogContent v-if="secret" class="k-dialog" :aria-describedby="undefined">
        <DialogTitle>{{ t("admin.secretTitle") }}</DialogTitle>
        <p>{{ t("admin.secretOnce") }}</p>
        <p>
          <code class="k-mono" :aria-label="t('admin.secretValue')">{{ secret.value }}</code>
        </p>
        <p v-if="secret.expires_at" class="k-muted">
          {{ t("admin.secretExpires", { at: secret.expires_at }) }}
        </p>
        <DialogClose as-child>
          <button type="button">{{ t("admin.secretDone") }}</button>
        </DialogClose>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
