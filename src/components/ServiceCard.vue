<script setup lang="ts">
import { computed } from "vue";
import type { Service, ValetService } from "../types/valet";
import ActionButton from "./ActionButton.vue";

const props = defineProps<{ service: Service; globalPhp: string | null }>();

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

/**
 * Service key understood by `valet start|stop|restart`. The CLI only manages the global PHP-FPM
 * (`php`); FPM pools of isolated versions are restarted by `valet isolate` itself.
 */
const key = computed<ValetService | null>(() => {
  const name = props.service.name;
  if (name === `php${props.globalPhp}`) return "php";
  return (["nginx", "dnsmasq", "mysql", "redis", "mailpit"] as const).find((s) => s === name) ?? null;
});
</script>

<template>
  <div
    class="flex flex-col rounded-lg border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900"
    :class="{ 'opacity-60': service.active === null }"
  >
    <div class="flex items-center justify-between gap-2">
      <p class="font-medium">{{ service.name }}</p>
      <span class="size-2.5 rounded-full" :class="status.dot" aria-hidden="true" />
    </div>
    <p class="mt-1 text-xs" :class="status.text">{{ status.label }}</p>
    <p class="mt-3 truncate font-mono text-xs text-zinc-500" :title="`${service.unit}.service`">{{ service.unit }}</p>

    <div v-if="key && service.active !== null" class="mt-3 flex flex-wrap gap-1.5">
      <ActionButton
        v-if="service.active"
        :action="{ kind: 'restart', service: key }"
        :options="{ label: `Restarting ${service.name}` }"
      >
        Restart
      </ActionButton>
      <ActionButton v-else :action="{ kind: 'start', service: key }" :options="{ label: `Starting ${service.name}` }">
        Start
      </ActionButton>
      <ActionButton
        v-if="service.active && key !== 'dnsmasq'"
        variant="danger"
        :action="{ kind: 'stop', service: key }"
        :options="{ label: `Stopping ${service.name}`, confirm: `Stop ${service.name}?` }"
      >
        Stop
      </ActionButton>
    </div>
    <p v-else-if="service.active !== null" class="mt-3 text-xs text-zinc-400">Managed by valet isolate</p>
  </div>
</template>
