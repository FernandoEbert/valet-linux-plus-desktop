<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { storeToRefs } from "pinia";
import { computed } from "vue";
import ActionButton from "../components/ActionButton.vue";
import { useActionsStore } from "../stores/actions";
import { useValetStore } from "../stores/valet";

const { state } = storeToRefs(useValetStore());
const actions = useActionsStore();

const sitesDir = computed(() => (state.value ? `${state.value.home}/Sites` : null));
const paths = computed(() =>
  (state.value?.paths ?? []).map((path) => ({
    path,
    // Valet's own links folder is parked by `valet install`; forgetting it breaks `valet link`.
    internal: path === sitesDir.value,
    sites: (state.value?.sites ?? []).filter((s) => s.type === "parked" && s.path?.startsWith(`${path}/`)).length,
  })),
);

async function park(): Promise<void> {
  const folder = await open({ directory: true, multiple: false, title: "Choose a folder to park" });
  if (typeof folder === "string") {
    await actions.run({ kind: "park", path: folder }, { label: `Parking ${folder}` });
  }
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex items-center justify-between gap-4">
      <p class="text-zinc-500">Every sub-folder of a parked path is served as <span class="font-mono">&lt;folder&gt;.{{ state?.domain }}</span>.</p>
      <button
        type="button"
        class="rounded-md border border-zinc-300 bg-white px-3 py-1.5 text-sm font-medium whitespace-nowrap hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:hover:bg-zinc-800"
        @click="park"
      >
        Park folder…
      </button>
    </div>

    <ul class="divide-y divide-zinc-100 rounded-lg border border-zinc-200 bg-white dark:divide-zinc-800 dark:border-zinc-800 dark:bg-zinc-900">
      <li v-for="item in paths" :key="item.path" class="flex items-center gap-4 px-4 py-3">
        <div class="min-w-0 flex-1">
          <p class="truncate font-mono text-[13px]" :title="item.path">{{ item.path }}</p>
          <p class="text-xs text-zinc-500">
            <template v-if="item.internal">Valet links folder (managed by <span class="font-mono">valet link</span>)</template>
            <template v-else>{{ item.sites }} site{{ item.sites === 1 ? "" : "s" }}</template>
          </p>
        </div>
        <button
          type="button"
          class="rounded px-2 py-1 text-xs text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800"
          @click="revealItemInDir(item.path)"
        >
          Show
        </button>
        <ActionButton
          v-if="!item.internal"
          variant="danger"
          :action="{ kind: 'forget', path: item.path }"
          :options="{
            label: `Forgetting ${item.path}`,
            confirm: `Stop serving the sites in ${item.path}? The folder is kept.`,
          }"
        >
          Forget
        </ActionButton>
      </li>
      <li v-if="!paths.length" class="px-4 py-6 text-center text-zinc-500">No parked paths.</li>
    </ul>
  </div>
</template>
