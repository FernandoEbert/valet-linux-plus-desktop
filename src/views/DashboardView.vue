<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed } from "vue";
import InfoItem from "../components/InfoItem.vue";
import ServiceCard from "../components/ServiceCard.vue";
import { useValetStore } from "../stores/valet";

const { state } = storeToRefs(useValetStore());

const siteCounts = computed(() => {
  const sites = state.value?.sites ?? [];
  return [
    { label: "Parked", value: sites.filter((s) => s.type === "parked").length },
    { label: "Linked", value: sites.filter((s) => s.type === "linked").length },
    { label: "Proxies", value: sites.filter((s) => s.type === "proxy").length },
    { label: "Secured", value: sites.filter((s) => s.secure).length },
    { label: "Isolated PHP", value: sites.filter((s) => s.isolated_php).length },
  ];
});

const cliMismatch = computed(() => {
  const php = state.value?.php;
  return !!php?.global && !!php.cli.version && !php.cli.version.startsWith(`${php.global}.`);
});
</script>

<template>
  <div v-if="state" class="flex flex-col gap-6">
    <section>
      <h2 class="mb-3 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Services</h2>
      <div class="grid grid-cols-[repeat(auto-fill,minmax(10.5rem,1fr))] gap-3">
        <ServiceCard v-for="service in state.services" :key="service.name" :service="service" />
      </div>
    </section>

    <section class="grid gap-4 lg:grid-cols-2">
      <div class="rounded-lg border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900">
        <h2 class="mb-3 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Valet</h2>
        <dl class="grid grid-cols-2 gap-x-4 gap-y-3">
          <InfoItem label="Version" :value="state.valet_version" />
          <InfoItem label="Data source" :value="state.source === 'cli' ? 'valet state --json' : 'config files'" />
          <InfoItem label="Domain" :value="`.${state.domain}`" mono />
          <InfoItem label="Ports (HTTP / HTTPS)" :value="`${state.port} / ${state.https_port}`" mono />
          <InfoItem class="col-span-2" label="Binary" :value="state.valet_path" mono />
          <InfoItem class="col-span-2" label="Home" :value="state.home" mono />
        </dl>
      </div>

      <div class="rounded-lg border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900">
        <h2 class="mb-3 text-xs font-semibold tracking-wide text-zinc-500 uppercase">PHP</h2>
        <dl class="grid grid-cols-2 gap-x-4 gap-y-3">
          <InfoItem label="Global (valet use)" :value="state.php.global" mono />
          <InfoItem label="CLI" :value="state.php.cli.version" :hint="state.php.cli.binary ?? state.php.cli.path" mono />
          <InfoItem class="col-span-2" label="Fallback binary" :value="state.php.fallback_binary" mono />
          <div class="col-span-2">
            <dt class="text-xs text-zinc-500">Installed</dt>
            <dd class="mt-1 flex flex-wrap gap-1.5">
              <span
                v-for="version in state.php.installed"
                :key="version"
                class="rounded border px-1.5 py-0.5 font-mono text-xs"
                :class="
                  version === state.php.global
                    ? 'border-emerald-400 bg-emerald-50 text-emerald-800 dark:border-emerald-800 dark:bg-emerald-950/40 dark:text-emerald-300'
                    : 'border-zinc-200 text-zinc-600 dark:border-zinc-700 dark:text-zinc-400'
                "
                >{{ version }}</span
              >
              <span v-if="!state.php.installed.length" class="text-zinc-500">—</span>
            </dd>
          </div>
        </dl>
        <p v-if="cliMismatch" class="mt-3 text-xs text-zinc-500">
          The CLI resolves a different PHP than the global FPM version; sites run on {{ state.php.global }} unless
          isolated.
        </p>
      </div>
    </section>

    <section>
      <h2 class="mb-3 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Sites</h2>
      <RouterLink
        to="/sites"
        class="grid grid-cols-5 divide-x divide-zinc-200 rounded-lg border border-zinc-200 bg-white hover:border-zinc-300 dark:divide-zinc-800 dark:border-zinc-800 dark:bg-zinc-900 dark:hover:border-zinc-700"
      >
        <div v-for="count in siteCounts" :key="count.label" class="px-4 py-3">
          <p class="text-xl font-semibold tabular-nums">{{ count.value }}</p>
          <p class="text-xs text-zinc-500">{{ count.label }}</p>
        </div>
      </RouterLink>
    </section>
  </div>
</template>
