<script setup lang="ts">
// An operation as a line (REQ-021-10, 11): its code in fixed pitch, what it names, then its
// other members — the line wraps beside its code, never cut (revue du 2026-10-25).
import type { Operation } from "@kartenn/sdk";
import { NoValue } from "@kartenn/ui";
import { computed } from "vue";
import { operationLine } from "./operations.ts";

const props = defineProps<{ operation: Operation | Record<string, unknown> }>();
const line = computed(() => operationLine(props.operation));
</script>

<template>
  <span class="k-operation">
    <code class="k-operation__op">{{ line.op }}</code>
    <span v-if="line.subject" class="k-mono k-operation__subject">{{ line.subject }}</span>
    <span v-for="member in line.members" :key="member.name" class="k-operation__member"
      >{{ member.name }} : <NoValue v-if="member.value === null" /><span v-else class="k-mono">{{
        member.value
      }}</span></span
    >
  </span>
</template>

<style>
@layer components {
  .k-operation {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1) var(--k-space-2);
    align-items: baseline;
    min-inline-size: 0;
  }

  .k-operation__op {
    flex: none;
  }

  .k-operation__subject,
  .k-operation__member {
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }

  .k-operation__member {
    color: var(--k-text-muted);
  }
}
</style>
