<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The machine column of the lists of addresses (point `column`, task 18ca): the machines
// whose interfaces hold the row's address, each a link to its record. The interfaces are
// followed (`useList`: read again once invalidated, and at other dates) from the turn of the
// cell, a few cells at a time (`limited`: a list mounts dozens of rows at once); the names
// of the machines through the shared cache (`Names`). Unreadable: said in a word, with the
// code of the problem (`CodeBadge`).
import { ApiProblem, reads, type ObjectRef } from "@kartenn/sdk";
import { CodeBadge, Skeleton } from "@kartenn/ui";
import { computed, onScopeDispose, shallowRef, watch } from "vue";
import { useList, useNotified } from "./data.ts";
import { hrefOf, PAGES } from "./dom.ts";
import { useHosted, useTexts } from "./host.ts";
import { limited, Names, type Interface } from "./reads.ts";

const props = defineProps<{
  object: ObjectRef | null;
  row: Record<string, unknown> | null;
}>();

const t = useTexts();
const { context } = useHosted();
const names = useNotified((changed, live) => new Names(changed, live));

/** The address of the row (or of the record). */
const address = computed(() => {
  const id = props.object?.id ?? props.row?.["id"];
  return typeof id === "string" ? id : null;
});

/** The address whose turn came: its interfaces are followed from then on. */
const turn = shallowRef<string | null>(null);
const list = useList<Interface, never>(() =>
  turn.value ? (reads.list("/v1/interfaces", { address: turn.value }) as never) : null,
);

let live = true;
/** Gives back the place of the cell among the reads at once (its first answer came). */
let release: (() => void) | null = null;
onScopeDispose(() => {
  live = false;
  release?.();
});
/** Counts the turns asked: a later one (another row) voids the former. */
let asked = 0;

watch(
  address,
  (id) => {
    release?.();
    turn.value = null;
    const ask = ++asked;
    if (!id) return;
    // The place is held until the first answer of the list (read or refused).
    void limited(
      () => live && ask === asked,
      () =>
        new Promise<void>((resolve) => {
          release = () => {
            release = null;
            resolve();
          };
          turn.value = id;
          if (!list.state.value.pending) release();
        }),
    );
  },
  { immediate: true },
);
watch(
  () => list.state.value.pending,
  (pending) => {
    if (!pending) release?.();
  },
);

/** The interfaces that hold it, `null` while they are read (at these dates). */
const found = computed(() => {
  const state = list.state.value;
  return state.items === undefined || state.previous ? null : state.items;
});
const failure = computed(() => (found.value === null ? list.state.value.error : null));

/** The machines of those interfaces, each once, with its name and the link to its record. */
const machines = computed(() => {
  // The links carry the dates of the context (W9).
  void context.value.time;
  const ids = [...new Set((found.value ?? []).map((iface) => iface.machine.id))];
  return ids.map((id) => ({
    id,
    name: names().of("machines.machine", id),
    href: hrefOf(PAGES.machine(id)),
  }));
});

const problem = computed(() => (failure.value instanceof ApiProblem ? failure.value : null));
</script>

<template>
  <span v-if="failure"
    >{{ t("column.unreadable") }}
    <CodeBadge v-if="problem" :code="problem.code" :label="t('code')" />
  </span>
  <Skeleton v-else-if="found === null" variant="cell" />
  <span v-else-if="machines.length === 0"
    ><span aria-hidden="true">—</span><span class="k-visually-hidden">{{ t("none") }}</span></span
  >
  <span v-else
    ><template v-for="(machine, index) in machines" :key="machine.id"
      ><template v-if="index > 0">, </template
      ><a v-if="machine.name.state === 'read'" :href="machine.href">{{ machine.name.name }}</a
      ><a v-else-if="machine.name.state === 'failed'" :href="machine.href" class="k-mono">{{
        machine.id
      }}</a
      ><Skeleton v-else variant="cell"
    /></template>
  </span>
</template>
