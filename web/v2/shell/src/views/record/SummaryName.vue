<script setup lang="ts">
// The name of an object the server summarised (REQ-006-42, task 021-11p10-d1): the object
// of an anomaly or of a gap, a node of an explanation, the value of a reference. Its
// `label`, a link to its record; nothing read, nor any read set up (no query, no watch of
// the view): a pure rendering of the summary. Without `label` but with its `href` (a type
// of module that tells no key), a link named by its identifier in fixed pitch. With
// neither (`label`, `slug` and `href` left out together: an object the caller may not read,
// or one gone), its identifier in fixed pitch, no link, not guessed at, said unreadable to
// a screen reader. An identifier alone is named by `ObjectName`, which reads it.
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import KLink from "../../prefetch/KLink.vue";
import { pageOfHref } from "../collections.ts";
import { useKeyLabel } from "../data.ts";
import { collectionOfRef } from "./objectRef.ts";

const props = defineProps<{
  summary: { id: string; kind?: string | null; label?: string | null; href?: string | null };
  /** The kind in words before the name (« Préfixe 192.0.2.0/24 »). */
  withKind?: boolean;
}>();
const { t } = useI18n();
const keyLabel = useKeyLabel();

const collection = computed(() => collectionOfRef(props.summary.kind, props.summary.href));
/** Its label; an empty one is none (nothing to name it by), as in the plugins. */
const label = computed(() => props.summary.label || null);
const unreadable = computed(() => label.value === null && !props.summary.href);
/** Its record: by its URL; else, named, by its kind. */
const to = computed(
  () =>
    pageOfHref(props.summary.href) ??
    (collection.value && label.value !== null
      ? `/${collection.value.id}/${props.summary.id}`
      : null),
);
const kindWords = computed(() =>
  collection.value ? (keyLabel("kind", collection.value.type) ?? collection.value.kind) : "",
);
</script>

<template>
  <span class="k-object-name">
    <!-- The space in the expression: the compiler trims one left at the end of a template. -->
    <template v-if="props.withKind && kindWords">{{ `${kindWords} ` }}</template>
    <KLink v-if="to" :to="to" :class="{ 'k-mono': label === null }">{{
      label ?? props.summary.id
    }}</KLink>
    <span v-else class="k-mono"
      >{{ label ?? props.summary.id
      }}<span v-if="unreadable" class="k-visually-hidden">{{
        ` (${t("views.record.objectUnreadable")})`
      }}</span></span
    >
  </span>
</template>
