<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, onMounted, onUnmounted, watch } from "vue";
import { RouterLink, RouterView, useRoute } from "vue-router";
import logoMark from "./assets/logo-mark.svg";
import ActionLog from "./components/ActionLog.vue";
import SourceNotice from "./components/SourceNotice.vue";
import { useActionsStore } from "./stores/actions";
import { useValetStore } from "./stores/valet";

const store = useValetStore();
const actions = useActionsStore();
const { sessionActive } = storeToRefs(actions);
const { state, error, loading, lastUpdated } = storeToRefs(store);
const route = useRoute();

const title = computed(() => String(route.meta.title ?? ""));
const updatedAt = computed(() => lastUpdated.value?.toLocaleTimeString() ?? "—");
const nav = computed(() => [
  { to: "/", label: "Dashboard", count: null },
  { to: "/sites", label: "Sites", count: state.value?.sites.length ?? null },
  { to: "/paths", label: "Paths", count: state.value?.paths.length ?? null },
  { to: "/php", label: "PHP", count: null },
  { to: "/logs", label: "Logs", count: null },
  { to: "/settings", label: "Settings", count: null },
]);

onMounted(() => {
  void store.start();
  void actions.refreshSession();
});
onUnmounted(() => store.stop());

// The privileged session may also be opened by the tray ("Restart all").
watch(lastUpdated, () => void actions.refreshSession());
</script>

<template>
  <div class="flex h-full">
    <aside class="flex w-52 shrink-0 flex-col border-r border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900">
      <div class="flex items-center gap-3 px-5 pt-5 pb-6">
        <img :src="logoMark" alt="" class="h-6 w-auto shrink-0" />
        <div class="min-w-0">
          <p class="text-base leading-tight font-semibold tracking-tight">Valet Linux+</p>
          <p class="text-xs text-zinc-500">{{ state?.valet_version ? `v${state.valet_version}` : "desktop" }}</p>
        </div>
      </div>
      <nav class="flex flex-col gap-0.5 px-2">
        <RouterLink
          v-for="item in nav"
          :key="item.to"
          :to="item.to"
          class="flex items-center justify-between rounded-md px-3 py-2 text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800"
          active-class="!bg-zinc-100 !text-zinc-900 font-medium dark:!bg-zinc-800 dark:!text-zinc-100"
        >
          <span>{{ item.label }}</span>
          <span v-if="item.count !== null" class="text-xs tabular-nums text-zinc-400">{{ item.count }}</span>
        </RouterLink>
      </nav>
    </aside>

    <div class="flex min-w-0 flex-1 flex-col">
      <header class="flex items-center justify-between gap-4 border-b border-zinc-200 px-6 py-3 dark:border-zinc-800">
        <h1 class="text-lg font-semibold">{{ title }}</h1>
        <div class="flex items-center gap-3">
          <span
            v-if="sessionActive"
            class="inline-flex items-center gap-2 rounded-full bg-amber-50 py-1 pr-1 pl-3 text-xs text-amber-800 ring-1 ring-amber-200 ring-inset dark:bg-amber-950/40 dark:text-amber-300 dark:ring-amber-900"
            title="Privileged actions won't ask for the password again until you end the session or close the app"
          >
            Admin session
            <button
              type="button"
              class="rounded-full px-2 py-0.5 font-medium hover:bg-amber-100 dark:hover:bg-amber-900/50"
              @click="actions.endSession()"
            >
              End
            </button>
          </span>
          <span class="text-xs text-zinc-500" title="Refreshes every 5 seconds and whenever ~/.config/valet changes">Updated {{ updatedAt }}</span>
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-md border border-zinc-300 bg-white px-3 py-1.5 text-sm font-medium hover:bg-zinc-50 disabled:opacity-60 dark:border-zinc-700 dark:bg-zinc-900 dark:hover:bg-zinc-800"
            :disabled="loading"
            @click="store.refresh(true)"
          >
            <svg
              class="size-4"
              :class="{ 'animate-spin': loading }"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              aria-hidden="true"
            >
              <path d="M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            Refresh
          </button>
        </div>
      </header>

      <main class="flex-1 overflow-y-auto px-6 py-5">
        <div
          v-if="error"
          class="mb-4 rounded-md border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-800 dark:border-red-900 dark:bg-red-950/50 dark:text-red-300"
        >
          <p class="font-medium">Could not read the Valet state</p>
          <p class="mt-0.5 font-mono text-xs">{{ error }}</p>
        </div>

        <SourceNotice v-if="state" :state="state" class="mb-4" />

        <RouterView v-if="state" />
        <p v-else-if="!error" class="text-zinc-500">Loading…</p>
      </main>
    </div>
    <ActionLog />
  </div>
</template>
