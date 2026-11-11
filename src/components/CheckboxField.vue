<script setup lang="ts">
const model = defineModel<boolean>({ required: true });
</script>

<template>
  <label class="group inline-flex cursor-pointer items-center gap-2 select-none">
    <span class="relative inline-flex size-4 shrink-0">
      <input
        v-model="model"
        type="checkbox"
        class="peer size-4 cursor-pointer appearance-none rounded border-[1.5px] border-zinc-400 bg-white transition-colors group-hover:border-zinc-600 checked:border-zinc-900 checked:bg-zinc-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-zinc-500 dark:border-zinc-500 dark:bg-zinc-900 dark:group-hover:border-zinc-300 dark:checked:border-white dark:checked:bg-white"
      />
      <svg
        class="pointer-events-none absolute inset-0 m-auto hidden size-3 text-white peer-checked:block dark:text-zinc-900"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
        aria-hidden="true"
      >
        <path d="M3.5 8.5l3 3 6-7" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </span>
    <span><slot /></span>
  </label>
</template>
