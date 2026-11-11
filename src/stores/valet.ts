import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { ValetState } from "../types/valet";

export const POLL_INTERVAL_MS = 5000;

export const useValetStore = defineStore("valet", () => {
  const state = ref<ValetState | null>(null);
  const error = ref<string | null>(null);
  const loading = ref(false);
  const lastUpdated = ref<Date | null>(null);

  let timer: ReturnType<typeof setInterval> | null = null;

  const isLimited = computed(() => state.value?.source === "files");
  const globalPhp = computed(() => state.value?.php.global ?? null);

  /** `force` waits for an in-flight read and reads again (used right after an action). */
  async function refresh(force = false): Promise<void> {
    // A slow `valet state` must not stack up calls from the poller.
    if (loading.value) {
      if (!force) return;
      while (loading.value) await new Promise((resolve) => setTimeout(resolve, 50));
    }

    loading.value = true;
    try {
      state.value = await invoke<ValetState>("get_state");
      error.value = null;
      lastUpdated.value = new Date();
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      loading.value = false;
    }
  }

  function startPolling(intervalMs = POLL_INTERVAL_MS): void {
    stopPolling();
    void refresh();
    timer = setInterval(() => {
      if (document.visibilityState === "visible") void refresh();
    }, intervalMs);
  }

  function stopPolling(): void {
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
  }

  return { state, error, loading, lastUpdated, isLimited, globalPhp, refresh, startPolling, stopPolling };
});
