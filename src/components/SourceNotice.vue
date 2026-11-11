<script setup lang="ts">
import { ref } from "vue";
import type { ValetState } from "../types/valet";

defineProps<{ state: ValetState }>();

const expanded = ref(false);
</script>

<template>
  <div
    v-if="state.source === 'files'"
    class="rounded-md border border-amber-300/70 bg-amber-50 px-4 py-2.5 text-sm text-amber-900 dark:border-amber-900/60 dark:bg-amber-950/30 dark:text-amber-200"
  >
    <div class="flex items-center justify-between gap-4">
      <p>
        Limited data: read straight from <span class="font-mono text-xs">~/.config/valet</span>. Update
        valet-linux-plus to 2.1.3+ for the full state.
      </p>
      <button
        v-if="state.warnings.length"
        type="button"
        class="shrink-0 text-xs underline underline-offset-2"
        @click="expanded = !expanded"
      >
        {{ expanded ? "Hide details" : "Details" }}
      </button>
    </div>
    <ul v-if="expanded" class="mt-2 list-disc pl-5 font-mono text-xs">
      <li v-for="warning in state.warnings" :key="warning">{{ warning }}</li>
    </ul>
  </div>
</template>
