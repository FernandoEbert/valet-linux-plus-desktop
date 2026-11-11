import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { ValetState } from "../types/valet";

/** Emitted by the backend monitor (src-tauri/src/app/monitor.rs) after every read. */
const STATE_EVENT = "valet://state";
const STATE_ERROR_EVENT = "valet://state-error";

export const useValetStore = defineStore("valet", () => {
  const state = ref<ValetState | null>(null);
  const error = ref<string | null>(null);
  const loading = ref(false);
  const lastUpdated = ref<Date | null>(null);

  let unlisten: UnlistenFn[] = [];

  const isLimited = computed(() => state.value?.source === "files");
  const globalPhp = computed(() => state.value?.php.global ?? null);

  function apply(next: ValetState): void {
    state.value = next;
    error.value = null;
    lastUpdated.value = new Date();
  }

  /**
   * Reads the state now. The backend monitor already pushes it every 5 s (30 s while hidden) and
   * on file changes; this is for the Refresh button and right after an action.
   * `force` waits for an in-flight read and reads again.
   */
  async function refresh(force = false): Promise<void> {
    if (loading.value) {
      if (!force) return;
      while (loading.value) await new Promise((resolve) => setTimeout(resolve, 50));
    }

    loading.value = true;
    try {
      apply(await invoke<ValetState>("get_state"));
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      loading.value = false;
    }
  }

  /** Subscribes to the backend monitor and loads the first state. */
  async function start(): Promise<void> {
    stop();
    unlisten = await Promise.all([
      listen<ValetState>(STATE_EVENT, (event) => apply(event.payload)),
      listen<string>(STATE_ERROR_EVENT, (event) => (error.value = event.payload)),
    ]);
    await refresh();
  }

  function stop(): void {
    unlisten.forEach((fn) => fn());
    unlisten = [];
  }

  return { state, error, loading, lastUpdated, isLimited, globalPhp, refresh, start, stop };
});
