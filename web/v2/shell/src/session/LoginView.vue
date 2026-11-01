<script setup lang="ts">
// The login page (REQ-021-14, REQ-007-28; design 007 « Session »): the issuers of
// `GET /v1/auth/issuers` (public), each a button that leaves for it, and the form of a
// break-glass account (`POST /v1/auth/breakglass`). Back, after the login, to the URL
// kept (`?return=`), its dates and filters included.
import type { ApiProblem } from "@kartenn/sdk";
import {
  Button,
  EmptyState,
  ErrorState,
  InlineAlert,
  Skeleton,
  Stack,
  TextField,
} from "@kartenn/ui";
import { computed, onMounted, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { loadSession } from "../rights/index.ts";
import { breakglassLogin, internalPath, loginAt, readIssuers } from "./index.ts";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
/** `undefined` while read, the problem when they could not be. */
const issuers = shallowRef<string[] | ApiProblem | undefined>(undefined);
const name = ref("");
const passphrase = ref("");
const code = ref("");
const refused = shallowRef<ApiProblem | null>(null);
/** What the refusal says: its message, its type and its Request-Id, each copyable. */
const refusal = computed(() => {
  const problem = refused.value;
  if (!problem) return [];
  return [
    {
      message: problem.detail ?? problem.title,
      ...(problem.type !== "about:blank" ? { code: problem.type } : {}),
    },
    ...(problem.requestId ? [{ message: "Request-Id", code: problem.requestId }] : []),
  ];
});
const busy = ref(false);

const back = (): string => internalPath(route.query.return) ?? "/";

async function read(): Promise<void> {
  issuers.value = undefined;
  issuers.value = await readIssuers();
}
onMounted(read);
/** The heading of the issuers: « Réessayer » goes away with its state, the focus goes here. */
const issuersHeading = ref<HTMLElement | null>(null);
function retry(): void {
  issuersHeading.value?.focus();
  void read();
}

function choose(issuer: string): void {
  loginAt(issuer, back());
}

async function breakglass(): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  refused.value = null;
  const problem = await breakglassLogin({
    name: name.value,
    passphrase: passphrase.value,
    code: code.value,
  });
  busy.value = false;
  // The proofs never stay in the page.
  passphrase.value = "";
  code.value = "";
  if (problem) {
    refused.value = problem;
    return;
  }
  await loadSession();
  await router.replace(back());
}
</script>

<template>
  <Stack gap="6" class="k-login">
    <h1 class="k-login__title">{{ t("login.title") }}</h1>
    <section aria-labelledby="k-login-issuers">
      <Stack gap="3">
        <h2 id="k-login-issuers" ref="issuersHeading" class="k-login__heading" tabindex="-1">
          {{ t("login.issuers") }}
        </h2>
        <Stack v-if="issuers === undefined" gap="2" aria-busy="true">
          <span class="k-visually-hidden">{{ t("login.loading") }}</span>
          <Skeleton variant="text" :rows="2" />
        </Stack>
        <ErrorState
          v-else-if="!Array.isArray(issuers)"
          :title="t('login.issuersFailed')"
          :problem="issuers"
          :retry-label="t('login.retry')"
          @retry="retry"
        />
        <EmptyState v-else-if="issuers.length === 0" :title="t('login.none')">
          {{ t("login.noneText") }}
        </EmptyState>
        <!-- The first issuer is the main way in; the others, and the break-glass account,
             secondary. -->
        <Stack v-else gap="2" align="start">
          <Button
            v-for="(issuer, i) in issuers"
            :key="issuer"
            :variant="i === 0 ? 'primary' : 'secondary'"
            @click="choose(issuer)"
          >
            {{ t("login.issuer", { id: issuer }) }}
          </Button>
        </Stack>
      </Stack>
    </section>
    <section aria-labelledby="k-login-breakglass">
      <form class="k-login__form" @submit.prevent="breakglass">
        <Stack gap="3">
          <h2 id="k-login-breakglass" class="k-login__heading">{{ t("login.breakglass") }}</h2>
          <p class="k-login__hint">{{ t("login.breakglassHint") }}</p>
          <TextField v-model="name" :label="t('login.name')" autocomplete="username" required />
          <TextField
            v-model="passphrase"
            type="password"
            :label="t('login.passphrase')"
            autocomplete="current-password"
            required
          />
          <TextField
            v-model="code"
            :label="t('login.code')"
            :hint="t('login.codeHint')"
            autocomplete="one-time-code"
            mono
            required
          />
          <!-- The refusal under the fields, its code and Request-Id copyable. -->
          <InlineAlert v-if="refused" tone="ko" :title="t('login.failed')" :items="refusal" />
          <div>
            <Button type="submit" :loading="busy">{{ t("login.submit") }}</Button>
          </div>
        </Stack>
      </form>
    </section>
  </Stack>
</template>

<style>
@layer components {
  .k-login {
    max-inline-size: var(--k-size-dialog-md);
  }

  .k-login__title {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-login__heading {
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-login__hint {
    color: var(--k-text-muted);
  }
}
</style>
