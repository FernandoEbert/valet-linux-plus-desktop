<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed } from "vue";
import { useActionsStore, type ActionEntry } from "../stores/actions";

const store = useActionsStore();
const { entries, pinned: expanded, deadlines } = storeToRefs(store);

/** Remaining time when the bar is (re)mounted; the CSS animation does the rest. */
function remainingMs(id: number): number {
  return Math.max(0, (deadlines.value[id] ?? 0) - Date.now());
}

// Only the latest few; older ones stay dismissed from view.
const visible = computed(() => entries.value.slice(0, 3));

function status(entry: ActionEntry): "running" | "ok" | "failed" {
  if (entry.result === null && entry.error === null) return "running";
  return entry.result?.success ? "ok" : "failed";
}

function output(entry: ActionEntry): string {
  if (entry.error) return entry.error;
  return [entry.result?.stdout, entry.result?.stderr].filter(Boolean).join("\n") || "(no output)";
}
</script>

<template>
  <div class="pointer-events-none fixed right-4 bottom-4 z-40 flex w-96 max-w-[calc(100vw-2rem)] flex-col gap-2">
    <div
      v-for="entry in visible"
      :key="entry.id"
      class="pointer-events-auto rounded-lg border bg-white text-sm shadow-lg dark:bg-zinc-900"
      :class="{
        'border-zinc-200 dark:border-zinc-700': status(entry) === 'running',
        'border-emerald-300 dark:border-emerald-900': status(entry) === 'ok',
        'border-red-300 dark:border-red-900': status(entry) === 'failed',
      }"
      role="status"
    >
      <div class="flex items-start gap-3 px-3 py-2.5">
        <span class="mt-1.5 size-2 shrink-0 rounded-full" :class="{
          'animate-pulse bg-zinc-400': status(entry) === 'running',
          'bg-emerald-500': status(entry) === 'ok',
          'bg-red-500': status(entry) === 'failed',
        }" />
        <div class="min-w-0 flex-1">
          <p class="font-medium">{{ entry.label }}</p>
          <p class="truncate font-mono text-xs text-zinc-500">
            {{ entry.result?.command ?? "" }}
            <template v-if="status(entry) === 'running'">running…<template v-if="!store.sessionActive && entry.action.kind !== 'park' && entry.action.kind !== 'forget' && entry.action.kind !== 'link' && entry.action.kind !== 'unlink'"> (asks for your password once per session)</template></template>
          </p>
        </div>
        <button
          v-if="status(entry) !== 'running'"
          type="button"
          class="text-xs text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-100"
          @click="store.togglePinned(entry.id)"
        >
          {{ expanded === entry.id ? "Hide" : "Output" }}
        </button>
        <button
          v-if="status(entry) !== 'running'"
          type="button"
          class="text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100"
          aria-label="Dismiss"
          @click="store.dismiss(entry.id)"
        >
          ×
        </button>
      </div>
      <p v-if="entry.error && expanded !== entry.id" class="px-3 pb-2.5 text-xs text-red-700 dark:text-red-400">
        {{ entry.error }}
      </p>
      <div v-if="deadlines[entry.id]" class="h-0.5 overflow-hidden rounded-b-lg" aria-hidden="true">
        <div
          :key="deadlines[entry.id]"
          class="countdown h-full"
          :class="entry.result?.success ? 'bg-emerald-400/70' : 'bg-red-400/70'"
          :style="{ animationDuration: `${remainingMs(entry.id)}ms` }"
        />
      </div>
      <pre
        v-if="expanded === entry.id"
        class="max-h-60 overflow-auto border-t border-zinc-200 px-3 py-2 font-mono text-xs whitespace-pre-wrap dark:border-zinc-800"
      >{{ output(entry) }}</pre>
    </div>
  </div>
</template>

<style scoped>
.countdown {
  transform-origin: left;
  animation-name: countdown;
  animation-timing-function: linear;
  animation-fill-mode: forwards;
}

@keyframes countdown {
  from {
    transform: scaleX(1);
  }
  to {
    transform: scaleX(0);
  }
}
</style>
