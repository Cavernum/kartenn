<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { problemText } from "../i18n/index.ts";
import { loadSession } from "../rights/index.ts";
import { internalPath, loginAt } from "../session/index.ts";

// Login page (REQ-007-28, design « Session »): the issuers of `GET /v1/auth/issuers`
// (public), and the break-glass form (`POST /v1/auth/breakglass`).

const i18n = useI18n();
const { t } = i18n;
const route = useRoute();
const router = useRouter();
const issuers = ref<string[] | null>(null);
const name = ref("");
const passphrase = ref("");
const code = ref("");
const error = ref<string | null>(null);
const busy = ref(false);

const back = (): string => internalPath(route.query.return) ?? "/";

onMounted(async () => {
  const response = await fetch("/v1/auth/issuers").catch(() => null);
  const body = response?.ok ? ((await response.json().catch(() => [])) as { id: string }[]) : [];
  issuers.value = body.map((issuer) => issuer.id);
});

function choose(issuer: string): void {
  loginAt(issuer, back());
}

async function breakglass(): Promise<void> {
  busy.value = true;
  error.value = null;
  const response = await fetch("/v1/auth/breakglass", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ name: name.value, passphrase: passphrase.value, code: code.value }),
  }).catch(() => null);
  busy.value = false;
  passphrase.value = "";
  code.value = "";
  if (response?.ok) {
    await loadSession();
    await router.replace(back());
    return;
  }
  const problem = (await response?.json().catch(() => null)) as {
    type?: string;
    title?: string;
    detail?: string;
  } | null;
  error.value = problem?.type
    ? problemText(i18n, {
        type: problem.type,
        title: problem.title ?? "",
        detail: problem.detail ?? null,
      })
    : t("login.failed");
}
</script>

<template>
  <h1>{{ t("login.title") }}</h1>
  <section aria-labelledby="k-login-issuers">
    <h2 id="k-login-issuers">{{ t("login.issuers") }}</h2>
    <p v-if="issuers === null" class="k-muted">{{ t("app.loading") }}</p>
    <p v-else-if="issuers.length === 0" class="k-muted">{{ t("login.none") }}</p>
    <ul v-else class="k-login-issuers">
      <li v-for="issuer in issuers" :key="issuer">
        <button type="button" @click="choose(issuer)">
          {{ t("login.issuer", { id: issuer }) }}
        </button>
      </li>
    </ul>
  </section>
  <section aria-labelledby="k-login-breakglass">
    <h2 id="k-login-breakglass">{{ t("login.breakglass") }}</h2>
    <p class="k-muted">{{ t("login.breakglassHint") }}</p>
    <form class="k-form" @submit.prevent="breakglass">
      <label>
        {{ t("login.name") }}
        <input v-model="name" name="username" autocomplete="username" required />
      </label>
      <label>
        {{ t("login.passphrase") }}
        <input
          v-model="passphrase"
          type="password"
          name="password"
          autocomplete="current-password"
          minlength="15"
          required
        />
      </label>
      <label>
        {{ t("login.code") }}
        <input
          v-model="code"
          name="one-time-code"
          autocomplete="one-time-code"
          inputmode="numeric"
          pattern="[0-9]{6}"
          required
        />
      </label>
      <p v-if="error" role="alert" class="k-error">{{ error }}</p>
      <button type="submit" :disabled="busy">{{ t("login.submit") }}</button>
    </form>
  </section>
</template>
