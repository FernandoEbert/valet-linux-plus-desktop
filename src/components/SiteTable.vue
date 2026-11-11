<script setup lang="ts">
import { ref } from "vue";
import type { Site, SiteType } from "../types/valet";
import SiteActions from "./SiteActions.vue";

defineProps<{ sites: Site[]; phpVersions: string[] }>();

const openError = ref<string | null>(null);

const typeClass: Record<SiteType, string> = {
  parked: "bg-sky-50 text-sky-700 ring-sky-200 dark:bg-sky-950/40 dark:text-sky-300 dark:ring-sky-900",
  linked: "bg-violet-50 text-violet-700 ring-violet-200 dark:bg-violet-950/40 dark:text-violet-300 dark:ring-violet-900",
  proxy: "bg-amber-50 text-amber-700 ring-amber-200 dark:bg-amber-950/40 dark:text-amber-300 dark:ring-amber-900",
  other: "bg-zinc-50 text-zinc-600 ring-zinc-200 dark:bg-zinc-800 dark:text-zinc-300 dark:ring-zinc-700",
};

function phpSource(site: Site): string | null {
  if (site.isolated_php) return "isolated";
  if (site.phprc_php) return ".valetphprc";
  return null;
}

</script>

<template>
  <div>
    <p v-if="openError" class="mb-2 text-xs text-red-600 dark:text-red-400">{{ openError }}</p>
    <div class="overflow-x-auto rounded-lg border border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900">
      <table class="w-full text-left">
        <thead class="border-b border-zinc-200 text-xs text-zinc-500 dark:border-zinc-800">
          <tr>
            <th class="px-4 py-2 font-medium">URL</th>
            <th class="px-3 py-2 font-medium">Type</th>
            <th class="px-3 py-2 font-medium">PHP</th>
            <th class="px-3 py-2 font-medium">SSL</th>
            <th class="px-3 py-2 font-medium">Folder / target</th>
            <th class="px-3 py-2 font-medium">Nginx</th>
            <th class="px-3 py-2"><span class="sr-only">Actions</span></th>
          </tr>
        </thead>
        <tbody class="divide-y divide-zinc-100 dark:divide-zinc-800">
          <tr v-for="site in sites" :key="site.domain" class="hover:bg-zinc-50 dark:hover:bg-zinc-800/50">
            <td class="px-4 py-2 font-mono text-[13px] whitespace-nowrap">{{ site.url }}</td>
            <td class="px-3 py-2 whitespace-nowrap">
              <span class="rounded px-1.5 py-0.5 text-xs ring-1 ring-inset" :class="typeClass[site.type]">
                {{ site.type }}
              </span>
            </td>
            <td class="px-3 py-2 whitespace-nowrap">
              <span class="font-mono text-[13px]">{{ site.php ?? "—" }}</span>
              <span v-if="phpSource(site)" class="ml-1.5 text-xs text-zinc-500">{{ phpSource(site) }}</span>
            </td>
            <td class="px-3 py-2">
              <span v-if="site.secure" class="text-emerald-600 dark:text-emerald-400" title="Secured">●</span>
              <span v-else class="text-zinc-300 dark:text-zinc-600" title="Not secured">○</span>
            </td>
            <td class="w-full max-w-0 truncate px-3 py-2 font-mono text-xs text-zinc-600 dark:text-zinc-400" :title="site.path ?? site.proxy_host ?? ''">
              {{ site.path ?? site.proxy_host ?? "—" }}
            </td>
            <td class="px-3 py-2 text-xs">
              <span v-if="site.custom_nginx" class="text-zinc-700 dark:text-zinc-300">custom</span>
              <span v-else class="text-zinc-400">—</span>
            </td>
            <td class="px-3 py-2 text-right whitespace-nowrap">
              <SiteActions :site="site" :php-versions="phpVersions" @error="openError = $event" />
            </td>
          </tr>
          <tr v-if="!sites.length">
            <td colspan="7" class="px-4 py-6 text-center text-zinc-500">No sites match.</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
