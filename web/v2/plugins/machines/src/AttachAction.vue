<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The action of an address (point `action`, task 18ca): attached to an interface of a chosen
// machine, as one operation added to the active draft (`draft.add`, W10) — never a
// changeset of its own. The machine is searched by its name (`Combobox`, `q` of
// `/v1/machines`), its interfaces then listed (`Select`, the first one taken). Required
// fields are refused on themselves before anything is sent; a refusal of the server is an
// alert with its codes (`CodeBadge`), each violation also on its field. Offered at the
// present only (W9); without the permission, the button stays, refused with its reason.
import {
  ApiProblem,
  draft,
  MAX_PAGES,
  reads,
  type Entry,
  type ObjectRef,
  type Operation,
} from "@kartenn/sdk";
import {
  Button,
  Checkbox,
  Cluster,
  Combobox,
  InlineAlert,
  Select,
  Stack,
  type ListOption,
  type Suggest,
} from "@kartenn/ui";
import { computed, nextTick, ref, shallowRef, useTemplateRef, watch } from "vue";
import { useList } from "./data.ts";
import { useClose, useHosted, usePresent, useTexts } from "./host.ts";
import type { Interface, Machine } from "./reads.ts";

const props = defineProps<{
  object: ObjectRef | null;
  row: Record<string, unknown> | null;
}>();

/** What the operation asks of the user. */
const PERMISSION = "machines.interface.edit";
/** The longest text `q` takes (the contract of `/v1/machines`). */
const SEARCH_MAX = 100;

const t = useTexts();
const { context, queryClient } = useHosted();
const present = usePresent();
const close = useClose();
const form = useTemplateRef<HTMLFormElement>("form");

const allowed = computed(() => context.value.user.has(PERMISSION));
const search = ref("");
const machine = ref<string | undefined>();
const iface = ref<string | undefined>();
const primary = ref<boolean | "mixed">(false);
/** The refusals said on the fields, by the name of the member of the operation. */
const errors = shallowRef<{ machine?: string | undefined; interface?: string | undefined }>({});
const adding = ref(false);
/** The outcome of the last addition, gone once the form is changed again. */
const said = ref("");
const refusal = shallowRef<unknown>(null);

const byName = (a: { name: string }, b: { name: string }) => a.name.localeCompare(b.name);

/** The machines whose name starts with the text (all of them without text), by name. */
const suggest: Suggest = async (text) => {
  const q = text.trim().slice(0, SEARCH_MAX);
  const data = (await queryClient.fetchInfiniteQuery(
    reads.list("/v1/machines", q ? { q } : {}) as never,
  )) as { pages: Entry<{ items: Machine[] }>[] };
  return data.pages
    .flatMap((page) => page.data.items)
    .sort(byName)
    .map((m): ListOption => ({ value: m.id, label: m.name }));
};

const interfaces = useList<Interface, never>(() =>
  machine.value ? (reads.list("/v1/interfaces", { machine: machine.value }) as never) : null,
);
// Every page of them, as many as the cache holds (`MAX_PAGES`); beyond, said in the help.
watch(
  () => interfaces.state.value,
  (list) => {
    if (list.more && !list.fetching && !list.previous && list.pages < MAX_PAGES) {
      interfaces.next();
    }
  },
  { immediate: true },
);
/** The interfaces of the machine chosen, never those of the former one kept meanwhile. */
const options = computed(() => {
  const list = interfaces.state.value;
  return list.previous
    ? []
    : [...(list.items ?? [])]
        .sort(byName)
        .map((i): ListOption => ({ value: i.id, label: i.name, detail: i.mac ?? undefined }));
});
// Another machine, or its interfaces read: the first one taken, unless one of them is chosen.
watch(options, (now) => {
  if (!now.some((o) => o.value === iface.value)) iface.value = now[0]?.value;
});

/** Why the field of the interface has nothing to offer: its help, read with it. */
const why = computed(() => {
  if (!machine.value) return t("action.chooseMachine");
  const list = interfaces.state.value;
  if (list.previous) return undefined;
  if (list.items?.length === 0) return t("action.noInterface");
  return list.more && list.pages >= MAX_PAGES ? t("action.firstInterfaces") : undefined;
});
/** Shown in the field while its interfaces are read. */
const placeholder = computed(() => {
  const list = interfaces.state.value;
  return machine.value && (list.pending || list.previous) ? t("loading") : undefined;
});

// The outcome of the last addition, or its refusal, no longer holds once a field changes.
watch([machine, iface, primary], () => {
  said.value = "";
  refusal.value = null;
});
// A field changed is no longer refused (what was said of it no longer holds).
watch(machine, () => (errors.value = { ...errors.value, machine: undefined }));
watch(iface, () => (errors.value = { ...errors.value, interface: undefined }));

// The field of the interface is mounted again as its options come (below): the focus it
// held goes to the new one rather than to the page.
const ifaceField = useTemplateRef<HTMLElement>("ifaceField");
watch(
  () => options.value.length,
  async () => {
    const field = ifaceField.value;
    if (!field?.contains(document.activeElement)) return;
    await nextTick();
    field.querySelector<HTMLElement>("[role='combobox']")?.focus();
  },
  { flush: "pre" },
);

/** The lines of a refusal: each rule broken with its code, the problem's, its request. */
function linesOf(problem: unknown): { message: string; code?: string }[] {
  if (!(problem instanceof ApiProblem)) return [{ message: t("network") }];
  const lines: { message: string; code?: string }[] = problem.violations.map((v) => ({
    message: v.detail,
    code: v.rule,
  }));
  if (problem.detail) lines.unshift({ message: problem.detail });
  lines.push({ message: t("code"), code: problem.code });
  if (problem.requestId) lines.push({ message: t("request"), code: problem.requestId });
  return lines;
}

const listFailure = computed(() => interfaces.state.value.error);

/** The first field said refused takes the focus, once drawn. */
async function focusRefused(): Promise<void> {
  await nextTick();
  form.value?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus();
}

async function submit(): Promise<void> {
  if (!present() || adding.value || !props.object) return;
  if (!allowed.value) {
    // Enter in a field submits too: the refusal said, as the button's tooltip says it.
    said.value = `${t("form.needs")} ${PERMISSION}`;
    return;
  }
  said.value = "";
  refusal.value = null;
  const required = t("form.required");
  // A text typed but no suggestion taken: said so. The interface is refused only once a
  // machine is chosen: before, its field says why.
  const noMachine = search.value.trim() ? t("action.pick") : required;
  errors.value = machine.value
    ? iface.value
      ? {}
      : { interface: required }
    : { machine: noMachine };
  if (!machine.value || !iface.value) return focusRefused();
  const operation = {
    op: "machines.attach_address",
    interface: { id: iface.value },
    address: { id: props.object.id },
    primary: primary.value === true,
  } as Operation;
  adding.value = true;
  // Said while it is sent: `aria-busy` on a button is seldom read.
  said.value = t("form.adding");
  try {
    const outcome = await draft.add(operation);
    if (outcome.kind === "saved") {
      said.value = t("form.added", { n: outcome.draft.operation_count });
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
  if (!(problem instanceof ApiProblem)) return;
  const next: { machine?: string; interface?: string } = {};
  for (const violation of problem.violations) {
    const name = violation.pointer?.split("/").pop();
    if (name === "interface" && !next.interface) {
      next.interface = `${violation.detail} (${violation.rule})`;
    }
  }
  errors.value = next;
  if (next.interface) void focusRefused();
}
</script>

<template>
  <Stack v-if="!present()" gap="4">
    <InlineAlert tone="info" :title="t('form.dated')" />
    <Cluster gap="2">
      <Button @click="close">{{ t("action.close") }}</Button>
    </Cluster>
  </Stack>
  <form v-else ref="form" novalidate @submit.prevent="submit">
    <Stack gap="4">
      <Combobox
        v-model="search"
        v-model:value="machine"
        :label="t('action.machine')"
        :suggest="suggest"
        :error="errors.machine"
        required
      />
      <InlineAlert
        v-if="listFailure"
        tone="ko"
        :title="t('interfaces.failed')"
        :items="linesOf(listFailure)"
      />
      <!-- Mounted again as its options come: `SelectValue` shows only the label of an
           option mounted before the value (skill kartenn-ui, « Pièges de Reka UI »). -->
      <div ref="ifaceField">
        <Select
          :key="`${machine ?? ''}:${options.length}`"
          v-model="iface"
          :label="t('action.interface')"
          :options="options"
          :placeholder="placeholder"
          :hint="why"
          :readonly="options.length === 0"
          :error="errors.interface"
          required
        />
      </div>
      <Checkbox v-model="primary" :label="t('action.primary')" />
      <InlineAlert
        v-if="refusal !== null"
        tone="ko"
        :title="t('form.refused')"
        :items="linesOf(refusal)"
      />
      <Cluster gap="2">
        <Button
          variant="primary"
          type="submit"
          :loading="adding"
          :forbidden="allowed ? undefined : PERMISSION"
          >{{ t("form.submit") }}</Button
        >
        <Button @click="close">{{ t("action.close") }}</Button>
        <!-- There from the start, so that what it holds is said. -->
        <span role="status">{{ said }}</span>
      </Cluster>
    </Stack>
  </form>
</template>
