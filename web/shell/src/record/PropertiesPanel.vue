<script setup lang="ts">
import { ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { read, type Problem } from "../data/api.ts";
import { nameOf, pageOf, valueText } from "../data/collections.ts";
import { problemText } from "../i18n/index.ts";
import { datedSearch, useTime } from "../time/index.ts";

// The resolved properties of an object and, on demand, the explanation of each one:
// where its value comes from and the chain walked (REQ-007-17, REQ-003-14, 15). The nodes
// of an explanation are `{kind, id, href}`: their names are read from their records.

interface Node {
  kind: string;
  id: string;
  href: string;
}
interface Explanation {
  property: string;
  value: unknown;
  masked: boolean;
  origin: Node | null;
  chain: (Node | null)[];
  provenance: string | null;
}

const props = defineProps<{ href: string }>();
const i18n = useI18n();
const { t } = i18n;
const time = useTime();

const values = shallowRef<[string, unknown][]>([]);
const masked = shallowRef<string[]>([]);
const problem = shallowRef<Problem | null>(null);
const explained = ref<Record<string, Explanation | Problem>>({});
/** Names of the nodes of the explanations, by URL. */
const names = ref<Record<string, string>>({});

watch(
  [() => props.href, () => time.value.validAt, () => time.value.asOf],
  async () => {
    explained.value = {};
    const answer = await read<{ properties: Record<string, unknown>; masked: string[] }>(
      `${props.href}/properties${datedSearch(time.value)}`,
    );
    if (!answer.ok) {
      problem.value = answer.problem;
      values.value = [];
      return;
    }
    problem.value = null;
    values.value = Object.entries(answer.body.properties).sort(([a], [b]) => a.localeCompare(b));
    masked.value = answer.body.masked;
  },
  { immediate: true },
);

async function explain(key: string): Promise<void> {
  const dated = datedSearch(time.value);
  const answer = await read<Explanation>(
    `${props.href}/properties/${encodeURIComponent(key)}/explanation${dated}`,
  );
  if (answer.ok) {
    const hrefs = new Set(
      [answer.body.origin, ...answer.body.chain]
        .filter((node): node is Node => node !== null)
        .map((node) => node.href)
        .filter((href) => !(href in names.value)),
    );
    const read_ = await Promise.all(
      [...hrefs].map(
        async (href) => [href, await read<Record<string, unknown>>(href + dated)] as const,
      ),
    );
    const found: Record<string, string> = {};
    for (const [href, record] of read_) if (record.ok) found[href] = nameOf(record.body);
    names.value = { ...names.value, ...found };
  }
  explained.value = { ...explained.value, [key]: answer.ok ? answer.body : answer.problem };
}

function isProblem(value: Explanation | Problem): value is Problem {
  return "status" in value;
}

function nodeName(node: Node | null): string {
  if (!node) return t("record.hidden");
  return `${t(`object.types.${node.kind}`)} ${names.value[node.href] ?? node.id}`;
}
</script>

<template>
  <p v-if="problem" role="alert" class="k-error">{{ problemText(i18n, problem) }}</p>
  <p v-else-if="values.length === 0" class="k-muted">{{ t("record.noProperties") }}</p>
  <dl v-else class="k-properties">
    <template v-for="[key, value] in values" :key="key">
      <dt>{{ key }}</dt>
      <dd>
        <span class="k-value">{{ valueText(value) }}</span>
        <button
          v-if="!explained[key]"
          type="button"
          :aria-label="t('record.explainKey', { key })"
          @click="explain(key)"
        >
          {{ t("record.explain") }}
        </button>
        <template v-else>
          <p v-if="isProblem(explained[key]!)" class="k-error">
            {{ problemText(i18n, explained[key] as Problem) }}
          </p>
          <div v-else class="k-explanation">
            <p>
              {{ t("record.origin") }}
              <RouterLink
                v-if="pageOf((explained[key] as Explanation).origin?.href)"
                :to="pageOf((explained[key] as Explanation).origin?.href)!"
              >
                {{ nodeName((explained[key] as Explanation).origin) }}
              </RouterLink>
              <template v-else>{{ nodeName((explained[key] as Explanation).origin) }}</template>
            </p>
            <p>{{ t("record.chain") }}</p>
            <ol>
              <li v-for="(node, index) in (explained[key] as Explanation).chain" :key="index">
                <RouterLink v-if="pageOf(node?.href)" :to="pageOf(node?.href)!">
                  {{ nodeName(node) }}
                </RouterLink>
                <template v-else>{{ nodeName(node) }}</template>
              </li>
            </ol>
          </div>
        </template>
      </dd>
    </template>
  </dl>
  <p v-if="masked.length > 0" class="k-muted">{{ t("record.masked", { n: masked.length }) }}</p>
</template>
