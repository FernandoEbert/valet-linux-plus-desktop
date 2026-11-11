<script setup lang="ts">
defineProps<{ label: string; value: string | null | undefined; mono?: boolean; hint?: string | null }>();
</script>

<template>
  <div class="min-w-0">
    <dt class="text-xs text-zinc-500">{{ label }}</dt>
    <dd class="mt-0.5 truncate" :class="{ 'font-mono text-[13px]': mono }" :title="value ?? undefined">
      {{ value || "—" }}
    </dd>
    <dd v-if="hint" class="truncate font-mono text-xs text-zinc-500" :title="hint">{{ hint }}</dd>
  </div>
</template>
