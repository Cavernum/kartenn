<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The machine column of the lists of addresses (point `column`, task 18ca): the machines
// whose interfaces hold the row's address, each a link to its record. The interfaces are
// followed (`useList`: read again once invalidated, and at other dates), read with those of
// the other rows of the window (`heldBy`, task 021-11p10-e: a list mounts dozens of rows at
// once); each machine by the summary of the interface (`iface.machine`, task 11p10-d4),
// nothing read. Unreadable: said in a word, with the code of the problem (`CodeBadge`).
import { ApiProblem, type ObjectRef } from "@kartenn/sdk";
import { CodeBadge, NoValue, Skeleton } from "@kartenn/ui";
import { computed } from "vue";
import CitedName from "./CitedName.vue";
import { useList } from "./data.ts";
import { PAGES } from "./dom.ts";
import { heldBy } from "./holders.ts";
import { useTexts } from "./host.ts";
import { readable, type Interface, type Summary } from "./reads.ts";

const props = defineProps<{
  object: ObjectRef | null;
  row: Record<string, unknown> | null;
}>();

const t = useTexts();

/** The address of the row (or of the record). */
const address = computed(() => {
  const id = props.object?.id ?? props.row?.["id"];
  return typeof id === "string" ? id : null;
});

const list = useList<Interface, never>(() =>
  address.value ? (heldBy(address.value) as never) : null,
);

/** The interfaces that hold it, `null` while they are read (at these dates). */
const found = computed(() => {
  const state = list.state.value;
  return state.items === undefined || state.previous ? null : state.items;
});
const failure = computed(() => (found.value === null ? list.state.value.error : null));

/** The machines of those interfaces, each once, by its summary (a read one first). */
const machines = computed(() => {
  const byId = new Map<string, Summary>();
  for (const { machine } of found.value ?? []) {
    const held = byId.get(machine.id);
    if (!held || (!readable(held) && readable(machine))) byId.set(machine.id, machine);
  }
  return [...byId.values()];
});

const problem = computed(() => (failure.value instanceof ApiProblem ? failure.value : null));
</script>

<template>
  <span v-if="failure"
    >{{ t("column.unreadable") }}
    <CodeBadge v-if="problem" :code="problem.code" :label="t('code')" />
  </span>
  <Skeleton v-else-if="found === null" variant="cell" />
  <NoValue v-else-if="machines.length === 0" />
  <span v-else
    ><template v-for="(machine, index) in machines" :key="machine.id"
      ><template v-if="index > 0">, </template
      ><CitedName :summary="machine" :page="PAGES.machine(machine.id)"
    /></template>
  </span>
</template>
