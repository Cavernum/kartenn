<script setup lang="ts">
import { computed, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { send, type Problem } from "../data/api.ts";
import { problemText } from "../i18n/index.ts";
import { useSession } from "../rights/index.ts";
import { useWrites } from "../time/index.ts";
import { actionsOf, operationText, type Action, type Changeset } from "./changesets.ts";

// The record of a changeset (REQ-007-18): state, operations, violations, counted
// differential, and the actions the user is offered (validation, rejection, withdrawal,
// undo), each sent with the state read as `If-Match`.

const i18n = useI18n();
const { t, locale } = i18n;
const route = useRoute();
const session = useSession();
const writes = useWrites();

const id = computed(() => String(route.params.id));
const changeset = shallowRef<Changeset | null>(null);
const etag = ref<string | null>(null);
const problem = shallowRef<Problem | null>(null);
const outcome = shallowRef<Problem | null>(null);
const busy = ref(false);
const motive = ref("");
/** The changeset an undo submitted. */
const undone = shallowRef<{ id: string; state: string } | null>(null);

async function load(): Promise<void> {
  const answer = await send<Changeset>("GET", `/v1/changesets/${id.value}`);
  if (answer.ok) {
    changeset.value = answer.body;
    etag.value = answer.etag;
    problem.value = null;
  } else {
    changeset.value = null;
    problem.value = answer.problem;
  }
}
watch(
  id,
  () => {
    undone.value = null;
    outcome.value = null;
    void load();
  },
  { immediate: true },
);

const actions = computed(() =>
  changeset.value
    ? actionsOf(changeset.value, session.value.me?.identity ?? null, writes.value.quickEdit)
    : [],
);

async function act(action: Action): Promise<void> {
  if (!etag.value) return;
  busy.value = true;
  const body = action === "rejection" ? { motive: motive.value } : undefined;
  const answer = await send<{ changeset: { id: string }; state: string }>(
    "POST",
    `/v1/changesets/${id.value}/${action}`,
    body,
    { "if-match": etag.value },
  );
  busy.value = false;
  outcome.value = answer.ok ? null : answer.problem;
  if (answer.ok && action === "undo" && answer.body) {
    undone.value = { id: answer.body.changeset.id, state: answer.body.state };
  }
  if (answer.ok) motive.value = "";
  await load();
}

function when(iso: string | null | undefined): string {
  if (!iso) return "—";
  return new Intl.DateTimeFormat(locale.value, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(iso),
  );
}

const you = computed(() => session.value.me?.identity ?? null);
const person = (value: string | null | undefined): string =>
  !value ? "—" : value === you.value ? t("changeset.you") : value;
</script>

<template>
  <p v-if="problem" role="alert" class="k-error">{{ problemText(i18n, problem) }}</p>
  <template v-else-if="changeset">
    <h1>{{ t("changeset.title", { motive: changeset.motive }) }}</h1>
    <p role="status" class="k-changeset-state">
      {{ t("changeset.stateIs", { state: t(`changeset.states.${changeset.state}`) }) }}
    </p>
    <dl class="k-fields">
      <dt>{{ t("changeset.kind") }}</dt>
      <dd>{{ t(`changeset.kinds.${changeset.kind}`) }}</dd>
      <dt>{{ t("changeset.author") }}</dt>
      <dd>{{ person(changeset.author) }}</dd>
      <dt>{{ t("changeset.submitted_at") }}</dt>
      <dd>
        <time :datetime="changeset.submitted_at">{{ when(changeset.submitted_at) }}</time>
      </dd>
      <dt>{{ t("changeset.valid_from") }}</dt>
      <dd>{{ when(changeset.valid_from) }}</dd>
      <template v-if="changeset.applied_at">
        <dt>{{ t("changeset.applied_at") }}</dt>
        <dd>{{ when(changeset.applied_at) }}</dd>
      </template>
      <template v-if="changeset.expires_at">
        <dt>{{ t("changeset.expires_at") }}</dt>
        <dd>{{ when(changeset.expires_at) }}</dd>
      </template>
      <template v-if="changeset.validator">
        <dt>{{ t("changeset.validator") }}</dt>
        <dd>{{ person(changeset.validator) }}</dd>
      </template>
      <template v-if="changeset.rejection">
        <dt>{{ t("changeset.rejection") }}</dt>
        <dd>{{ changeset.rejection }}</dd>
      </template>
      <dt>{{ t("field.id") }}</dt>
      <dd>{{ changeset.id }}</dd>
    </dl>

    <section v-if="actions.length > 0" :aria-label="t('changeset.actions')">
      <h2>{{ t("changeset.actions") }}</h2>
      <div class="k-toolbar">
        <button
          v-if="actions.includes('validation')"
          type="button"
          :disabled="busy"
          @click="act('validation')"
        >
          {{ t("changeset.validate") }}
        </button>
        <button
          v-if="actions.includes('withdrawal')"
          type="button"
          :disabled="busy"
          @click="act('withdrawal')"
        >
          {{ t("changeset.withdraw") }}
        </button>
        <button v-if="actions.includes('undo')" type="button" :disabled="busy" @click="act('undo')">
          {{ t("changeset.undo") }}
        </button>
      </div>
      <form
        v-if="actions.includes('rejection')"
        class="k-inline-form"
        :aria-label="t('changeset.reject')"
        @submit.prevent="act('rejection')"
      >
        <label>
          {{ t("changeset.rejectMotive") }}
          <input v-model="motive" required />
        </label>
        <button type="submit" :disabled="busy">{{ t("changeset.reject") }}</button>
      </form>
    </section>
    <p v-if="outcome" role="alert" class="k-error">{{ problemText(i18n, outcome) }}</p>
    <p v-if="undone" role="status">
      {{ t("changeset.undoneBy") }}
      <RouterLink :to="`/changesets/${undone.id}`">{{
        t(`changeset.states.${undone.state}`)
      }}</RouterLink>
    </p>

    <h2>{{ t("changeset.operations") }}</h2>
    <ol class="k-operations">
      <li v-for="(operation, index) in changeset.operations" :key="index">
        <strong>{{ operationText(operation).op }}</strong>
        <code class="k-mono">{{ operationText(operation).rest }}</code>
      </li>
    </ol>

    <template v-if="changeset.violations.length > 0">
      <h2>{{ t("changeset.violations") }}</h2>
      <ul>
        <li v-for="(violation, index) in changeset.violations" :key="index">
          <strong>{{ violation.rule }}</strong> ({{ violation.severity }}) — {{ violation.detail }}
        </li>
      </ul>
    </template>

    <h2>{{ t("changeset.diff") }}</h2>
    <p v-if="changeset.diff.length === 0" class="k-muted">{{ t("changeset.noDiff") }}</p>
    <table v-else class="k-table">
      <caption class="k-visually-hidden">
        {{
          t("changeset.diff")
        }}
      </caption>
      <thead>
        <tr>
          <th scope="col">{{ t("changeset.property") }}</th>
          <th scope="col">{{ t("changeset.before") }}</th>
          <th scope="col">{{ t("changeset.after") }}</th>
          <th scope="col">{{ t("changeset.count") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(line, index) in changeset.diff" :key="index">
          <td>{{ line.property }}</td>
          <td>{{ line.masked ? t("changeset.masked") : (line.before ?? "—") }}</td>
          <td>{{ line.masked ? t("changeset.masked") : (line.after ?? "—") }}</td>
          <td>{{ line.count }}</td>
        </tr>
      </tbody>
    </table>
  </template>
</template>
