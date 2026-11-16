<script setup lang="ts">
// The name of an object known by its identifier alone (a value typed in a draft, a column
// of references still without summary): its kind in words and its name, read from its
// record through the cache at the dates of the page, a link to that record. Until it is
// read, its identifier in fixed pitch. A reference the server summarised is named by
// `SummaryName`, which reads nothing (task 021-11p10-d1). A cell of a list mounts one per
// row: it reads only while it is in view, through the gate of the shell (`data/gate.ts`,
// task 021-11p10-a), its read under way given up when it leaves the view and nothing else
// in view wants it.
import { reads } from "@kartenn/sdk";
import { computed, useTemplateRef } from "vue";
import { giveUpUnseen, limited } from "../../data/gate.ts";
import { useInView } from "../../data/inView.ts";
import { useQuery } from "../../data/query.ts";
import KLink from "../../prefetch/KLink.vue";
import { useTime } from "../../time/index.ts";
import { nameOf, pageOfHref, type Item } from "../collections.ts";
import { useKeyLabel } from "../data.ts";
import { collectionOfRef } from "./objectRef.ts";

const props = defineProps<{
  id: string;
  kind?: string | null | undefined;
  href?: string | null | undefined;
  /** The kind in words before the name (« Préfixe 192.0.2.0/24 »). */
  withKind?: boolean;
}>();
const time = useTime();
const keyLabel = useKeyLabel();

/** The kind: given, else that of the collection its URL names. */
const collection = computed(() => collectionOfRef(props.kind, props.href));
const options = () =>
  reads.object(collection.value?.kind ?? "prefix", props.id, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  });
const inView = useInView(useTemplateRef<HTMLElement>("root"), () =>
  giveUpUnseen(options().queryKey),
);
const record = useQuery(() => {
  const read = options();
  return {
    ...read,
    queryFn: (context: Parameters<typeof read.queryFn>[0]) =>
      limited(context.signal, () => read.queryFn(context)),
    enabled: collection.value !== undefined && inView.value,
  };
});
const name = computed(() => {
  const data = record.data.value?.data as Item | undefined;
  return data ? nameOf(data) : null;
});
const to = computed(
  () => pageOfHref(props.href) ?? (collection.value ? `/${collection.value.id}/${props.id}` : null),
);
const kindWords = computed(() =>
  collection.value ? (keyLabel("kind", collection.value.type) ?? collection.value.kind) : "",
);
</script>

<template>
  <span ref="root" class="k-object-name">
    <!-- The space in the expression: the compiler trims one left at the end of a template. -->
    <template v-if="props.withKind && kindWords">{{ `${kindWords} ` }}</template>
    <KLink v-if="to" :to="to" :class="{ 'k-mono': name === null }">{{ name ?? props.id }}</KLink>
    <span v-else class="k-mono">{{ name ?? props.id }}</span>
  </span>
</template>
