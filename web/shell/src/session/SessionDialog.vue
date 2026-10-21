<script setup lang="ts">
import {
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { expired, lastIssuer, loginAt } from "./index.ts";

// A 401 while in use (REQ-007-28): « Session expirée », and a login again that comes back
// to the same URL, dates included; the draft is in IndexedDB, nothing is lost.

const { t } = useI18n();
const route = useRoute();
const router = useRouter();

function reconnect(): void {
  const issuer = lastIssuer();
  if (issuer) {
    loginAt(issuer, route.fullPath);
    return;
  }
  expired.value = false;
  void router.push({ name: "login", query: { return: route.fullPath } });
}
</script>

<template>
  <DialogRoot :open="expired" @update:open="(value) => (expired = value)">
    <DialogPortal>
      <DialogOverlay class="k-overlay" />
      <DialogContent class="k-dialog" role="alertdialog">
        <DialogTitle>{{ t("session.expired") }}</DialogTitle>
        <DialogDescription>{{ t("session.text") }}</DialogDescription>
        <div class="k-toolbar">
          <button type="button" @click="reconnect">{{ t("session.reconnect") }}</button>
          <DialogClose>{{ t("session.later") }}</DialogClose>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
