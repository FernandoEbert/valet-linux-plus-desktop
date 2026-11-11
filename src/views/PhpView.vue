<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, ref } from "vue";
import ActionButton from "../components/ActionButton.vue";
import CheckboxField from "../components/CheckboxField.vue";
import InfoItem from "../components/InfoItem.vue";
import { useValetStore } from "../stores/valet";

const { state } = storeToRefs(useValetStore());
const updateCli = ref(false);

const versions = computed(() => {
  const php = state.value?.php;
  const sites = state.value?.sites ?? [];
  return (php?.installed ?? []).map((version) => ({
    version,
    global: version === php?.global,
    cli: !!php?.cli.version?.startsWith(`${version}.`),
    sites: sites.filter((s) => s.php === version).length,
  }));
});

const isolated = computed(() => (state.value?.sites ?? []).filter((s) => s.isolated_php || s.phprc_php));
</script>

<template>
  <div v-if="state" class="flex flex-col gap-6">
    <section class="grid gap-4 rounded-lg border border-zinc-200 bg-white p-4 sm:grid-cols-3 dark:border-zinc-800 dark:bg-zinc-900">
      <InfoItem label="Global (served by Valet)" :value="state.php.global" mono />
      <InfoItem label="CLI (php in PATH)" :value="state.php.cli.version" :hint="state.php.cli.binary ?? state.php.cli.path" mono />
      <InfoItem label="Fallback binary" :value="state.php.fallback_binary" mono />
    </section>

    <section>
      <div class="mb-3 flex items-center justify-between gap-4">
        <h2 class="text-xs font-semibold tracking-wide text-zinc-500 uppercase">Installed versions</h2>
        <CheckboxField v-model="updateCli" class="text-xs text-zinc-600 dark:text-zinc-400">
          "Use" also switches the CLI <span class="font-mono">php</span> (<span class="font-mono">--update-cli</span>)
        </CheckboxField>
      </div>
      <ul class="divide-y divide-zinc-100 rounded-lg border border-zinc-200 bg-white dark:divide-zinc-800 dark:border-zinc-800 dark:bg-zinc-900">
        <li v-for="item in versions" :key="item.version" class="flex items-center gap-4 px-4 py-3">
          <span class="w-12 font-mono text-[15px] font-medium">{{ item.version }}</span>
          <div class="flex flex-1 flex-wrap items-center gap-1.5">
            <span v-if="item.global" class="rounded bg-emerald-50 px-1.5 py-0.5 text-xs text-emerald-700 ring-1 ring-emerald-200 ring-inset dark:bg-emerald-950/40 dark:text-emerald-300 dark:ring-emerald-900">global</span>
            <span v-if="item.cli" class="rounded bg-zinc-100 px-1.5 py-0.5 text-xs text-zinc-700 ring-1 ring-zinc-200 ring-inset dark:bg-zinc-800 dark:text-zinc-300 dark:ring-zinc-700">CLI</span>
            <span class="text-xs text-zinc-500">{{ item.sites }} site{{ item.sites === 1 ? "" : "s" }}</span>
          </div>
          <div class="flex items-center gap-1.5">
            <ActionButton
              v-if="!item.cli"
              :action="{ kind: 'use_cli', php: item.version }"
              :options="{ label: `Switching the CLI php to ${item.version}` }"
              title="Only the php command in your terminal; sites keep their PHP"
            >
              CLI
            </ActionButton>
            <ActionButton
              v-if="!item.global || (updateCli && !item.cli)"
              :action="{ kind: 'use', php: item.version, update_cli: updateCli }"
              :options="{
                label: `Switching Valet${updateCli ? ' and the CLI' : ''} to PHP ${item.version}`,
                confirm: `Serve every non-isolated site with PHP ${item.version}${updateCli ? ' and switch the CLI php too' : ''}?`,
              }"
            >
              Use
            </ActionButton>
          </div>
        </li>
        <li v-if="!versions.length" class="px-4 py-6 text-center text-zinc-500">No versioned PHP binaries found in /usr/bin.</li>
      </ul>
      <p class="mt-2 text-xs text-zinc-500">
        <span class="font-mono">Use</span> changes the PHP Valet serves (<span class="font-mono">valet use</span>, which only
        accepts the versions it supports; older ones can still be used per site with Isolate, Sites → ⋯).
        <span class="font-mono">CLI</span> changes only the <span class="font-mono">php</span> command in the terminal
        (<span class="font-mono">update-alternatives</span>).
      </p>
    </section>

    <section>
      <h2 class="mb-3 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Sites with their own PHP</h2>
      <div class="overflow-hidden rounded-lg border border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900">
        <table class="w-full text-left">
          <thead class="border-b border-zinc-200 text-xs text-zinc-500 dark:border-zinc-800">
            <tr>
              <th class="px-4 py-2 font-medium">Site</th>
              <th class="px-3 py-2 font-medium">Effective</th>
              <th class="px-3 py-2 font-medium">Isolated</th>
              <th class="px-3 py-2 font-medium">.valetphprc</th>
              <th class="px-3 py-2"><span class="sr-only">Actions</span></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-zinc-100 dark:divide-zinc-800">
            <tr v-for="site in isolated" :key="site.domain">
              <td class="px-4 py-2 font-mono text-[13px]">{{ site.domain }}</td>
              <td class="px-3 py-2 font-mono text-[13px]">{{ site.php ?? "—" }}</td>
              <td class="px-3 py-2 font-mono text-[13px]">{{ site.isolated_php ?? "—" }}</td>
              <td class="px-3 py-2 font-mono text-[13px]">{{ site.phprc_php ?? "—" }}</td>
              <td class="px-3 py-2 text-right">
                <ActionButton
                  v-if="site.isolated_php"
                  :action="{ kind: 'unisolate', site: site.name }"
                  :options="{ label: `Unisolating ${site.name}` }"
                >
                  Use global
                </ActionButton>
              </td>
            </tr>
            <tr v-if="!isolated.length">
              <td colspan="5" class="px-4 py-6 text-center text-zinc-500">Every site uses the global PHP.</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </div>
</template>
