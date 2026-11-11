<script setup lang="ts">
const model = defineModel<boolean>({ required: true });
defineProps<{ label: string; description?: string; disabled?: boolean }>();
</script>

<template>
  <label class="flex items-start justify-between gap-6 py-3" :class="disabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'">
    <span class="min-w-0">
      <span class="block font-medium">{{ label }}</span>
      <span v-if="description" class="mt-0.5 block text-xs text-zinc-500">{{ description }}</span>
    </span>
    <span class="relative mt-0.5 inline-flex shrink-0">
      <input v-model="model" type="checkbox" role="switch" class="peer sr-only" :disabled="disabled" />
      <span
        class="h-5 w-9 rounded-full border-[1.5px] border-zinc-400 bg-zinc-200 transition-colors peer-checked:border-zinc-900 peer-checked:bg-zinc-900 peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-zinc-500 dark:border-zinc-500 dark:bg-zinc-800 dark:peer-checked:border-white dark:peer-checked:bg-white"
        aria-hidden="true"
      />
      <span
        class="absolute top-[3px] left-[3px] size-3.5 rounded-full bg-white shadow transition-transform peer-checked:translate-x-4 dark:bg-zinc-400 dark:peer-checked:bg-zinc-900"
        aria-hidden="true"
      />
    </span>
  </label>
</template>
