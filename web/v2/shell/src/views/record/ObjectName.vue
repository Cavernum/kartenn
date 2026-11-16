<script setup lang="ts">
// The name of an object an answer only references, a link to its record, its kind in words
// before it if asked. Summarised by the server (`summary`, REQ-006-42: `{id, kind?, label?,
// href?}`, the object of an anomaly, a node of an explanation, the value of a reference):
// its `label`, nothing read (task 021-11p10-d1); a summary without one names an object the
// caller may not read or one gone, its identifier in fixed pitch, not guessed at, said so
// to a screen reader, linked only by the `href` a summary may still carry. An
// identifier alone (`id`, a value typed in a draft): its name read from its record through
// the cache at the dates of the page, its identifier in fixed pitch until then. A cell of
// a list mounts one per row: it reads only while it is in view, through the gate of the
// shell (`data/gate.ts`, task 021-11p10-a), its read under way given up when it leaves the
// view and nothing else in view wants it.
import { reads } from "@kartenn/sdk";
import { computed, useTemplateRef } from "vue";
import { useI18n } from "vue-i18n";
import { giveUpUnseen, limited } from "../../data/gate.ts";
import { useInView } from "../../data/inView.ts";
import { useQuery } from "../../data/query.ts";
import KLink from "../../prefetch/KLink.vue";
import { useTime } from "../../time/index.ts";
import { collectionById, collectionOfKind, nameOf, pageOfHref, type Item } from "../collections.ts";
import { useKeyLabel } from "../data.ts";

const props = defineProps<{
  /** The reference as the server summarised it: named by its `label`, never read. */
  summary?: { id: string; kind?: string | null; label?: string | null; href?: string | null };
  /** An identifier alone, without `summary`: its name read from its record. */
  id?: string | undefined;
  kind?: string | null | undefined;
  href?: string | null | undefined;
  /** The kind in words before the name (« Préfixe 192.0.2.0/24 »). */
  withKind?: boolean;
}>();
const { t } = useI18n();
const time = useTime();
const keyLabel = useKeyLabel();

const target = computed(
  () => props.summary ?? { id: props.id ?? "", kind: props.kind, href: props.href, label: null },
);
/** The kind: given, else that of the collection its URL names. */
const collection = computed(
  () =>
    collectionOfKind(target.value.kind) ??
    collectionById(/\/v1\/([a-z-]+)\//.exec(target.value.href ?? "")?.[1]),
);
const options = () =>
  reads.object(collection.value?.kind ?? "prefix", target.value.id, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  });
const inView = useInView(useTemplateRef<HTMLElement>("root"), () => {
  if (props.summary === undefined) giveUpUnseen(options().queryKey);
});
const record = useQuery(() => {
  const read = options();
  return {
    ...read,
    queryFn: (context: Parameters<typeof read.queryFn>[0]) =>
      limited(context.signal, () => read.queryFn(context)),
    enabled: props.summary === undefined && collection.value !== undefined && inView.value,
  };
});
const name = computed(() => {
  if (props.summary !== undefined) return props.summary.label ?? null;
  const data = record.data.value?.data as Item | undefined;
  return data ? nameOf(data) : null;
});
/** Summarised without a label: unreadable or gone, said in words to a screen reader. */
const unreadable = computed(() => props.summary !== undefined && name.value === null);
/** Its record: by its URL; else, unless the summary says it unreadable, by its kind. */
const to = computed(
  () =>
    pageOfHref(target.value.href) ??
    (collection.value && !unreadable.value ? `/${collection.value.id}/${target.value.id}` : null),
);
const kindWords = computed(() =>
  collection.value ? (keyLabel("kind", collection.value.type) ?? collection.value.kind) : "",
);
</script>

<template>
  <span ref="root" class="k-object-name">
    <!-- The space in the expression: the compiler trims one left at the end of a template. -->
    <template v-if="props.withKind && kindWords">{{ `${kindWords} ` }}</template>
    <KLink v-if="to" :to="to" :class="{ 'k-mono': name === null }"
      >{{ name ?? target.id
      }}<span v-if="unreadable" class="k-visually-hidden">{{
        ` (${t("views.record.objectUnreadable")})`
      }}</span></KLink
    >
    <span v-else class="k-mono"
      >{{ name ?? target.id
      }}<span v-if="unreadable" class="k-visually-hidden">{{
        ` (${t("views.record.objectUnreadable")})`
      }}</span></span
    >
  </span>
</template>
