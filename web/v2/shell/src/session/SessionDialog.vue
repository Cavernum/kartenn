<script setup lang="ts">
// A 401 while in use (REQ-021-14, REQ-007-28): « Session expirée », and a login again that
// comes back to the same URL — its dates and filters included. The drafts are at the server
// (024): nothing is lost. Straight to the issuer chosen last, else the login page.
import { AlertDialog } from "@kartenn/ui";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { expired, lastIssuer, loginAt } from "./index.ts";

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
  void router.push({ path: "/login", query: { return: route.fullPath } });
}
</script>

<template>
  <AlertDialog
    v-model:open="expired"
    :title="t('session.expired')"
    :description="t('session.text')"
    :confirm="t('session.reconnect')"
    :cancel="t('session.later')"
    @confirm="reconnect"
  />
</template>
