<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { storeToRefs } from "pinia";
import { computed, ref } from "vue";
import LinkDialog from "../components/LinkDialog.vue";
import SiteTable from "../components/SiteTable.vue";
import { useValetStore } from "../stores/valet";

const { state } = storeToRefs(useValetStore());
const query = ref("");
const linkPath = ref<string | null>(null);

const sites = computed(() => {
  const all = state.value?.sites ?? [];
  const terms = query.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return all;

  return all.filter((site) => {
    const haystack = [site.name, site.domain, site.path, site.proxy_host, site.type, site.php]
      .filter(Boolean)
      .join(" ")
      .toLowerCase();
    return terms.every((term) => haystack.includes(term));
  });
});

async function pickFolder(): Promise<void> {
  const folder = await open({ directory: true, multiple: false, title: "Choose a folder to link" });
  if (typeof folder === "string") linkPath.value = folder;
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex items-center gap-3">
      <input
        v-model="query"
        type="search"
        placeholder="Filter by name, domain, folder, PHP…"
        class="w-full max-w-md rounded-md border border-zinc-300 bg-white px-3 py-1.5 outline-none placeholder:text-zinc-400 focus:border-zinc-500 dark:border-zinc-700 dark:bg-zinc-900"
        autofocus
      />
      <span class="text-xs text-zinc-500 tabular-nums">{{ sites.length }} / {{ state?.sites.length ?? 0 }}</span>
      <button
        type="button"
        class="ml-auto rounded-md border border-zinc-300 bg-white px-3 py-1.5 text-sm font-medium whitespace-nowrap hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:hover:bg-zinc-800"
        @click="pickFolder"
      >
        Link folder…
      </button>
    </div>

    <SiteTable :sites="sites" :php-versions="state?.php.installed ?? []" />
    <LinkDialog :path="linkPath" :domain="state?.domain ?? 'test'" @close="linkPath = null" />
  </div>
</template>
