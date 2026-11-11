<script setup lang="ts">
import { computed } from "vue";
import type { Service } from "../types/valet";

const props = defineProps<{ service: Service }>();

const status = computed(() => {
  switch (props.service.active) {
    case true:
      return { label: "Running", dot: "bg-emerald-500", text: "text-emerald-700 dark:text-emerald-400" };
    case false:
      return { label: "Stopped", dot: "bg-red-500", text: "text-red-700 dark:text-red-400" };
    default:
      return { label: "Not installed", dot: "bg-zinc-300 dark:bg-zinc-600", text: "text-zinc-500" };
  }
});
</script>

<template>
  <div
    class="rounded-lg border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900"
    :class="{ 'opacity-60': service.active === null }"
  >
    <div class="flex items-center justify-between gap-2">
      <p class="font-medium">{{ service.name }}</p>
      <span class="size-2.5 rounded-full" :class="status.dot" aria-hidden="true" />
    </div>
    <p class="mt-1 text-xs" :class="status.text">{{ status.label }}</p>
    <p class="mt-3 truncate font-mono text-xs text-zinc-500" :title="`${service.unit}.service`">{{ service.unit }}</p>
  </div>
</template>
