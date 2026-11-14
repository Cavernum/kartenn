<script setup lang="ts">
// A secret of a service account, made then shown once (005, K8; REQ-007-19, REQ-021-12;
// maquette E7, vue `secret`): the creation of an account (its name, the expiry of its first
// secret, the motive) or a new secret of `account` (its expiry, the motive), written at
// once; then, in the same dialog, the secret: « Copiez-le maintenant : il ne sera plus
// jamais affiché », the code to copy, its expiry, « J'ai copié le secret ». The focus goes
// to the copy, never to the cross; Escape, the cross or a click outside ask first (closed
// by mistake, a secret shown once is lost), and do nothing while the write is in flight. The secret lives in this dialog only
// (`useSecretWrite`): closed, it is forgotten. Left open as the window narrows under
// 1024 px (the gate of writes), the form keeps its input and sends nothing, the reason in
// the dialog; a secret already shown stays readable and copyable (a read) until closed.
import {
  AlertDialog,
  Button,
  CodeBadge,
  DateTimeField,
  Dialog,
  InlineAlert,
  Stack,
  TextField,
} from "@kartenn/ui";
import { computed, nextTick, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import { ACCOUNT_NAME } from "./access.ts";
import { useSecretWrite } from "./secret.ts";

const props = defineProps<{
  /** The account a new secret is for; none: an account created. */
  account?: { id: string; name: string } | undefined;
  /** The names already taken (a creation refuses them). */
  taken?: readonly string[];
}>();
const open = defineModel<boolean>("open", { default: false });
const { t } = useI18n();
const write = useSecretWrite();
const formId = useId();
const body = ref<HTMLElement | null>(null);

const name = ref("");
const expires = ref<string | null>(null);
const motive = ref("");
const tried = ref(false);
const expiry = ref<InstanceType<typeof DateTimeField> | null>(null);
/** The step of the dialog: its form, then the secret (kept while the dialog fades out). */
const mode = ref<"form" | "secret">("form");
/** The account the secret is for (its name, not a secret). */
const who = ref("");
watch(open, (now) => {
  // Closed: the secret is forgotten, whatever closed it.
  if (!now) return write.forget();
  mode.value = "form";
  name.value = "";
  expires.value = null;
  motive.value = "";
  tried.value = false;
  write.refusal.value = null;
});

// Escape, the cross or a click outside while the secret is shown: asked first, since the
// secret is lost with the dialog; « J'ai copié le secret » closes at once.
const confirming = ref(false);
function onOpen(now: boolean): void {
  // The write in flight: nothing closes (its secret would come to a closed dialog).
  if (!now && write.busy.value) return;
  if (!now && mode.value === "secret" && write.secret.value) confirming.value = true;
  else open.value = now;
}

const shown = computed(() => write.secret.value);
const title = computed(() =>
  mode.value === "secret"
    ? t("admin.secret.title", { account: who.value })
    : props.account
      ? t("admin.secret.title", { account: props.account.name })
      : t("admin.accounts.createTitle"),
);
const nameError = computed(() => {
  if (props.account || !tried.value) return undefined;
  const text = name.value.trim();
  if (!text) return t("admin.required", { field: t("admin.accounts.name") });
  if (!ACCOUNT_NAME.test(text)) return t("admin.accounts.nameInvalid");
  return props.taken?.includes(text) ? t("admin.accounts.nameTaken") : undefined;
});
const motiveError = computed(() =>
  tried.value && !motive.value.trim()
    ? t("admin.required", { field: t("admin.motive") })
    : undefined,
);

/**
 * Why nothing is sent now: the window narrowed (or the period moved) since it opened. Said
 * on the form only: a secret already shown stays readable and copyable until closed. Not
 * while the write is in flight: sent before the gate shut, it goes on.
 */
const held = computed(() =>
  mode.value === "form" && !write.busy.value ? write.gate.why.value : undefined,
);

async function submit(): Promise<void> {
  // Refused by the gate, read when sending (`run` reads it again): said in the dialog, the
  // input kept.
  if (held.value) return;
  tried.value = true;
  const dated = expiry.value?.check() ?? true;
  if (nameError.value || motiveError.value || !dated) return;
  const why = motive.value.trim();
  const done = props.account
    ? await write.renew(props.account.id, props.account.name, expires.value, why)
    : await write.create(name.value.trim(), expires.value, why);
  if (!done) return;
  if (!write.secret.value) {
    open.value = false;
    return;
  }
  who.value = write.account.value;
  mode.value = "secret";
  // The form gone with the button that had the focus: the focus to the copy of the secret.
  await nextTick();
  body.value?.querySelector<HTMLElement>("[data-autofocus] button")?.focus();
}
const expiryDate = computed(() => shown.value?.expires_at?.slice(0, 10) ?? null);
</script>

<template>
  <Dialog :open="open" :title="title" size="md" @update:open="onOpen">
    <div ref="body">
      <!-- The secret step stays while the dialog fades out, its value already forgotten. -->
      <Stack v-if="mode === 'secret'" gap="3">
        <InlineAlert tone="warn" :title="t('admin.secret.once')">
          {{ t("admin.secret.onceText") }}
        </InlineAlert>
        <!-- Its name says it is shown once: the focus lands here, the alert is not read. -->
        <CodeBadge :code="shown?.value ?? ''" :label="t('admin.secret.value')" data-autofocus />
        <p class="k-admin-muted">
          <template v-if="expiryDate">
            {{ t("admin.secret.expires") }}
            <time :datetime="shown?.expires_at ?? undefined">{{ expiryDate }}</time>
          </template>
          <template v-else>{{ t("admin.secret.never") }}</template>
        </p>
      </Stack>
      <form v-else :id="formId" novalidate @submit.prevent="submit">
        <Stack gap="3">
          <TextField
            v-if="!props.account"
            v-model="name"
            :label="t('admin.accounts.name')"
            :hint="t('admin.accounts.nameHint')"
            :error="nameError"
            mono
            required
            autocomplete="off"
          />
          <DateTimeField
            ref="expiry"
            v-model="expires"
            :label="t('admin.secret.expiry')"
            :hint="t('admin.secret.expiryHint')"
          />
          <TextField
            v-model="motive"
            :label="t('admin.motive')"
            :error="motiveError"
            required
            autocomplete="off"
          />
          <InlineAlert v-if="held" tone="warn" icon="lock" :title="held" />
          <RefusalAlert v-if="write.refusal.value" :refusal="write.refusal.value" :note="null" />
        </Stack>
      </form>
    </div>
    <template #actions>
      <Button v-if="mode === 'secret'" variant="primary" icon="check" @click="open = false">{{
        t("admin.secret.copied")
      }}</Button>
      <template v-else>
        <Button variant="ghost" :disabled="write.busy.value" @click="onOpen(false)">{{
          t("admin.cancel")
        }}</Button>
        <Button
          variant="primary"
          :icon="props.account ? 'key-round' : 'plus'"
          type="submit"
          :form="formId"
          :loading="write.busy.value"
          v-bind="held ? { reason: held } : {}"
          >{{ props.account ? t("admin.secret.make") : t("admin.accounts.create") }}</Button
        >
      </template>
    </template>
    <AlertDialog
      v-model:open="confirming"
      :title="t('admin.secret.leaveTitle')"
      :description="t('admin.secret.leaveText')"
      :confirm="t('admin.secret.leaveConfirm')"
      danger
      @confirm="open = false"
    />
  </Dialog>
</template>
