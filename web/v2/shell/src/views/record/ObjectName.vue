<script setup lang="ts">
// The name of an object an answer only references (`{kind, id, href}`: a node of an
// explanation, the value of a reference): its kind in words and its name, read from its
// record through the cache at the dates of the page, a link to that record. Until it is
// read, its identifier in fixed pitch.
import { reads } from "@kartenn/sdk";
import { computed } from "vue";
import { useQuery } from "../../data/query.ts";
import KLink from "../../prefetch/KLink.vue";
import { useTime } from "../../time/index.ts";
import { collectionById, collectionOfKind, nameOf, pageOfHref, type Item } from "../collections.ts";
import { useKeyLabel } from "../data.ts";

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
const collection = computed(
  () =>
    collectionOfKind(props.kind) ?? collectionById(/\/v1\/([a-z-]+)\//.exec(props.href ?? "")?.[1]),
);
const record = useQuery(() => ({
  ...reads.object(collection.value?.kind ?? "prefix", props.id, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  }),
  enabled: collection.value !== undefined,
}));
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
  <span class="k-object-name">
    <template v-if="props.withKind && kindWords">{{ kindWords }} </template>
    <KLink v-if="to" :to="to" :class="{ 'k-mono': name === null }">{{ name ?? props.id }}</KLink>
    <span v-else class="k-mono">{{ name ?? props.id }}</span>
  </span>
</template>
