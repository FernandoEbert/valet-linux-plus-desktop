<script setup lang="ts">
import { computed } from "vue";
import { useActionsStore, type RunOptions } from "../stores/actions";
import type { Action } from "../types/valet";

const props = defineProps<{
  action: Action;
  options: RunOptions;
  variant?: "default" | "danger" | "primary";
  disabled?: boolean;
}>();
const emit = defineEmits<{ done: [success: boolean] }>();

const store = useActionsStore();
const running = computed(() => store.isPending(props.action));

const variants = {
  default:
    "border-zinc-300 bg-white text-zinc-700 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-300 dark:hover:bg-zinc-800",
  danger:
    "border-zinc-300 bg-white text-red-700 hover:bg-red-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-red-400 dark:hover:bg-red-950/40",
  primary:
    "border-zinc-900 bg-zinc-900 text-white hover:bg-zinc-700 dark:border-zinc-100 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-zinc-300",
};

async function onClick(): Promise<void> {
  emit("done", await store.run(props.action, props.options));
}
</script>

<template>
  <button
    type="button"
    class="inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-xs font-medium whitespace-nowrap disabled:cursor-not-allowed disabled:opacity-50"
    :class="variants[variant ?? 'default']"
    :disabled="disabled || running"
    @click="onClick"
  >
    <svg v-if="running" class="size-3 animate-spin" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="3" opacity="0.25" />
      <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
    </svg>
    <slot />
  </button>
</template>
