<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The search of the interfaces (task 18cc; design 021 « Plugin Machines », incident
// response): by MAC, address and VID, its criteria in the query of the page by a link the
// shell follows (a new entry of the history, found again by the back button). The address
// and the VID are first resolved to their objects (an address in several domains, a VID in
// several groups: one list of `/v1/interfaces` for each pair), the interfaces of the lists
// merged in one `DataGrid`, the next pages read as the window nears the end, never again
// after a page refused. What it found is said by a live region there from the start.
import { ApiProblem, MAX_PAGES, reads, type Entry } from "@kartenn/sdk";
import {
  Button,
  Cluster,
  EmptyState,
  ErrorState,
  Grid,
  Heading,
  InlineAlert,
  Stack,
  TextField,
  type GridWindow,
} from "@kartenn/ui";
import { computed, onScopeDispose, reactive, shallowRef, watch } from "vue";
import { useLists, useNotified } from "./data.ts";
import { followLink } from "./dom.ts";
import { useHosted, useTexts } from "./host.ts";
import InterfaceGrid from "./InterfaceGrid.vue";
import { Names, type Interface } from "./reads.ts";
import RetryAlert from "./RetryAlert.vue";

/** The rows left before the end of the window when the next pages are read. */
const AHEAD = 20;
const KEYS = ["mac", "address", "vid"] as const;

/** The criteria of the search, as the query of the page holds them. */
type Criteria = Record<(typeof KEYS)[number], string>;

const t = useTexts();
const { context, element, queryClient } = useHosted();
const names = useNotified((changed, live) => new Names(changed, live));

function criteriaOfPage(): Criteria {
  const query = new URLSearchParams(location.search);
  return {
    mac: query.get("mac") ?? "",
    address: query.get("address") ?? "",
    vid: query.get("vid") ?? "",
  };
}
const same = (a: Criteria, b: Criteria) => KEYS.every((key) => a[key] === b[key]);

/** The criteria searched, and those of the fields. */
const searched = shallowRef<Criteria>(criteriaOfPage());
const fields = reactive<Criteria>({ ...searched.value });

/** Searches the criteria of the fields, held in the query of the page; its dates kept. */
function run(): void {
  const next: Criteria = {
    mac: fields.mac.trim(),
    address: fields.address.trim(),
    vid: fields.vid.trim(),
  };
  const query = new URLSearchParams(location.search);
  for (const key of KEYS) {
    if (next[key]) query.set(key, next[key]);
    else query.delete(key);
  }
  if (query.toString() !== new URLSearchParams(location.search).toString()) {
    const text = query.toString();
    followLink(element, `${location.pathname}${text ? `?${text}` : ""}`);
  }
  searched.value = next;
}
function clear(): void {
  for (const key of KEYS) fields[key] = "";
  run();
}
// Back or forward to another query of the same page: its criteria, in the fields and searched.
const onPop = () => {
  // Another page (a record): the shell replaces the element, its search is not changed.
  if (location.pathname.replace(/\/$/, "") !== "/x/machines/interfaces") return;
  const now = criteriaOfPage();
  Object.assign(fields, now);
  if (!same(now, searched.value)) searched.value = now;
};
window.addEventListener("popstate", onPop);
onScopeDispose(() => window.removeEventListener("popstate", onPop));

// ---------------------------------------------------------------------------- resolution

/** The pages read, at most, to resolve an address or a VID (the objects beyond: said). */
const RESOLVE_PAGES = 3;
/** The lists of `/v1/interfaces` read, at most, for one search (the pairs beyond: said). */
const MAX_LISTS = 12;

/**
 * The address and VLAN of each list to read, and what was left out of the search (said, never
 * dropped in silence); or what the resolution failed or found.
 */
type Resolved =
  | { pairs: { address?: string | undefined; vlan?: string | undefined }[]; left: string[] }
  | { problem: unknown }
  | { nothing: string };
/** `null` while the address and the VID are resolved. */
const resolved = shallowRef<Resolved | null>(null);
let generation = 0;

type Pages = { pages: Entry<{ items: { id: string }[]; links: { next?: string | null } }>[] };
/**
 * The identifiers of the objects whose `filter` is `value`, from their first `RESOLVE_PAGES`
 * pages; `more` if some are left beyond them.
 */
async function idsOf(path: "/v1/addresses" | "/v1/vlans", filter: string, value: string) {
  const options = { ...(reads.list(path, { [filter]: value }) as object), pages: RESOLVE_PAGES };
  const more = (data: Pages) => !!data.pages.at(-1)?.data.links.next;
  let data = (await queryClient.fetchInfiniteQuery(options as never)) as Pages;
  // Fresh in the cache with fewer pages (read by another screen): read again up to the bound.
  if (more(data) && data.pages.length < RESOLVE_PAGES) {
    data = (await queryClient.fetchInfiniteQuery({ ...options, staleTime: 0 } as never)) as Pages;
  }
  return {
    ids: data.pages.flatMap((page) => page.data.items.map((item) => item.id)),
    more: more(data),
  };
}

async function resolve(): Promise<void> {
  const mine = ++generation;
  const criteria = searched.value;
  resolved.value = null;
  const none = { ids: [undefined], more: false };
  try {
    const [addresses, vlans] = await Promise.all([
      criteria.address ? idsOf("/v1/addresses", "value", criteria.address) : none,
      criteria.vid ? idsOf("/v1/vlans", "vid", criteria.vid) : none,
    ]);
    if (mine !== generation) return;
    if (addresses.ids.length === 0) {
      resolved.value = { nothing: t("search.noAddress", { value: criteria.address }) };
    } else if (vlans.ids.length === 0) {
      resolved.value = { nothing: t("search.noVlan", { value: criteria.vid }) };
    } else {
      const left: string[] = [];
      if (addresses.more) {
        left.push(t("search.manyAddresses", { n: addresses.ids.length, value: criteria.address }));
      }
      if (vlans.more) {
        left.push(t("search.manyVlans", { n: vlans.ids.length, value: criteria.vid }));
      }
      const pairs = addresses.ids.flatMap((address) =>
        vlans.ids.map((vlan) => ({ address, vlan })),
      );
      if (pairs.length > MAX_LISTS) {
        left.push(t("search.manyLists", { total: pairs.length, n: MAX_LISTS }));
      }
      resolved.value = { pairs: pairs.slice(0, MAX_LISTS), left };
    }
  } catch (problem) {
    if (mine === generation) resolved.value = { problem };
  }
}
// Resolved again for other criteria, and at other dates.
watch(
  [searched, () => `${context.value.time.validAt ?? ""} ${context.value.time.asOf ?? ""}`],
  () => void resolve(),
  { immediate: true },
);

// --------------------------------------------------------------------------------- lists

const lists = useLists<Interface, never>(() => {
  const r = resolved.value;
  if (!r || !("pairs" in r)) return [];
  const { mac } = searched.value;
  return r.pairs.map(
    ({ address, vlan }) =>
      reads.list("/v1/interfaces", {
        ...(mac ? { mac } : {}),
        ...(address ? { address } : {}),
        ...(vlan ? { vlan } : {}),
      }) as never,
  );
});
const states = computed(() => lists.states.value);
/** The interfaces of every list, each once. */
const items = computed(() => {
  const seen = new Set<string>();
  return states.value
    .flatMap((list) => list.items ?? [])
    .filter((i) => !seen.has(i.id) && (seen.add(i.id), true));
});
const pending = computed(() => !resolved.value || states.value.some((l) => l.pending));
const error = computed(() => states.value.find((l) => l.error)?.error ?? null);
const problem = computed(() => {
  const r = resolved.value;
  return r && "problem" in r ? r.problem : null;
});
const nothing = computed(() => {
  const r = resolved.value;
  return r && "nothing" in r ? r.nothing : null;
});
/** Nothing read, and a list refused: the error of the view. */
const failure = computed(
  () => problem.value ?? (items.value.length === 0 && !pending.value ? error.value : null),
);
const more = computed(() => states.value.some((l) => l.more));
const capped = computed(() => states.value.some((l) => l.more && l.pages >= MAX_PAGES));
/** What the search left out: objects beyond the bound of the resolution, pairs beyond theirs. */
const left = computed(() => {
  const r = resolved.value;
  return r && "pairs" in r ? r.left : [];
});
/** The result is incomplete, and says so (the notice, and the sentence of the region). */
const partial = computed(() => left.value.length > 0 || capped.value);

function onWindow(window: GridWindow): void {
  if (window.end >= items.value.length - AHEAD) lists.next();
}
/** « Réessayer »: the resolution again, or the lists refused. */
function retry(): void {
  if (problem.value) void resolve();
  else lists.retry();
}
const problemOf = (e: unknown) => (e instanceof ApiProblem ? e : undefined);

/** What the search found, said by the live region. */
const sentence = computed(() => {
  if (nothing.value) return nothing.value;
  // Said by the alert of the error state: not twice.
  if (failure.value) return "";
  if (items.value.length === 0) return pending.value ? t("search.searching") : t("search.none");
  return t(partial.value ? "search.countPartial" : "search.count", { n: items.value.length });
});
</script>

<template>
  <Stack gap="6">
    <header>
      <Stack gap="1">
        <Heading :level="1" tabindex="-1">{{ t("interfaces.title") }}</Heading>
        <p>{{ t("interfaces.intro") }}</p>
      </Stack>
    </header>
    <form role="search" :aria-label="t('search.label')" novalidate @submit.prevent="run">
      <Stack gap="4">
        <Grid columns="auto" gap="4">
          <TextField
            v-model="fields.mac"
            :label="t('search.mac')"
            :hint="t('interfaces.macHint')"
            mono
          />
          <TextField v-model="fields.address" :label="t('search.address')" mono />
          <TextField v-model="fields.vid" :label="t('search.vid')" mono />
        </Grid>
        <Cluster gap="2">
          <Button variant="primary" type="submit" icon="search">{{ t("search.submit") }}</Button>
          <Button icon="x" @click="clear">{{ t("search.clear") }}</Button>
        </Cluster>
      </Stack>
    </form>
    <Stack gap="4">
      <!-- There from the start, so that each sentence it takes is said. -->
      <p role="status" class="k-visually-hidden">{{ sentence }}</p>
      <ErrorState
        v-if="failure"
        :title="t('interfaces.failed')"
        :problem="problemOf(failure)"
        :retry-label="t('retry')"
        @retry="retry"
      >
        {{ problemOf(failure) ? "" : t("network") }}
      </ErrorState>
      <EmptyState v-else-if="nothing" :title="nothing">
        {{ t("search.again") }}
        <template #action>
          <Button variant="ghost" icon="x" @click="clear">{{ t("search.clear") }}</Button>
        </template>
      </EmptyState>
      <EmptyState v-else-if="!pending && items.length === 0" :title="t('search.none')">
        {{ t("search.again") }}
        <template v-if="KEYS.some((key) => searched[key])" #action>
          <Button variant="ghost" icon="x" @click="clear">{{ t("search.clear") }}</Button>
        </template>
      </EmptyState>
      <template v-else>
        <RetryAlert
          v-if="error"
          :title="t('interfaces.failed')"
          :error="error"
          @retry="lists.retry()"
        />
        <InlineAlert
          v-if="left.length"
          tone="warn"
          :title="t('search.partial')"
          :items="left.map((message) => ({ message }))"
        />
        <InlineAlert v-if="capped" tone="info" :title="t('search.first', { n: items.length })" />
        <p v-if="items.length" aria-hidden="true">{{ sentence }}</p>
        <InterfaceGrid
          :label="t('interfaces.found')"
          :interfaces="items"
          :names="names"
          :loading="items.length === 0"
          :more="more || pending"
          @window="onWindow"
        />
      </template>
    </Stack>
  </Stack>
</template>
