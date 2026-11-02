<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The machine column of the lists of addresses (point `column`, task 18ca): the machines
// whose interfaces hold the row's address, each a link to its record. The interfaces are
// read once the cell is mounted (the grid mounts the rows in view), a few cells at a time
// (`limited`), again at other dates; the names of the machines through the shared cache
// (`Names`). Unreadable: said in a word, with the code of the problem (`CodeBadge`).
import { ApiProblem, reads, type Entry, type ObjectRef } from "@kartenn/sdk";
import { CodeBadge } from "@kartenn/ui";
import { computed, onScopeDispose, shallowRef, watch } from "vue";
import { useNotified } from "./data.ts";
import { hrefOf, PAGES } from "./dom.ts";
import { useHosted, useTexts } from "./host.ts";
import { limited, Names, type Interface } from "./reads.ts";

const props = defineProps<{
  object: ObjectRef | null;
  row: Record<string, unknown> | null;
}>();

const t = useTexts();
const { context, queryClient } = useHosted();
const names = useNotified((changed, live) => new Names(changed, live));

/** The address of the row (or of the record). */
const address = computed(() => {
  const id = props.object?.id ?? props.row?.["id"];
  return typeof id === "string" ? id : null;
});

/** The interfaces that hold it, `null` while they are read. */
const found = shallowRef<Interface[] | null>(null);
const failure = shallowRef<unknown>(null);
let live = true;
onScopeDispose(() => (live = false));
/** Counts the reads asked: a later one (another row, other dates) voids the former. */
let asked = 0;

watch(
  [address, () => context.value.time],
  ([id]) => {
    const ask = ++asked;
    found.value = null;
    failure.value = null;
    if (!id) return;
    const current = () => live && ask === asked;
    void limited(current, () =>
      queryClient.fetchInfiniteQuery(reads.list("/v1/interfaces", { address: id }) as never),
    ).then(
      (data) => {
        if (data === null || !current()) return;
        const pages = (data as { pages: Entry<{ items: Interface[] }>[] }).pages;
        found.value = pages.flatMap((page) => page.data.items);
      },
      (error: unknown) => {
        if (current()) failure.value = error;
      },
    );
  },
  { immediate: true },
);

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
  <span v-if="failure !== null"
    >{{ t("column.unreadable") }}
    <CodeBadge v-if="problem" :code="problem.code" :label="t('code')" />
  </span>
  <span v-else-if="found === null"
    ><span aria-hidden="true">…</span
    ><span class="k-visually-hidden">{{ t("loading") }}</span></span
  >
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
      ><span v-else
        ><span aria-hidden="true">…</span
        ><span class="k-visually-hidden">{{ t("loading") }}</span></span
      ></template
    ></span
  >
</template>
