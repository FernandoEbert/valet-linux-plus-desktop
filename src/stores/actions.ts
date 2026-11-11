import { invoke } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import { defineStore } from "pinia";
import { ref } from "vue";
import type { Action, ActionResult } from "../types/valet";
import { useValetStore } from "./valet";

export interface ActionEntry {
  id: number;
  action: Action;
  label: string;
  /** null while running */
  result: ActionResult | null;
  /** Error raised before/around the CLI call (validation, pkexec cancelled, valet missing). */
  error: string | null;
}

export interface RunOptions {
  /** Short description shown while running, e.g. "Securing shop". */
  label: string;
  /** When set, asks for confirmation first. */
  confirm?: string;
}

export const actionKey = (action: Action): string => JSON.stringify(action);

/** How long a finished action stays on screen. */
export const AUTO_DISMISS_MS = 30_000;

export const useActionsStore = defineStore("actions", () => {
  const entries = ref<ActionEntry[]>([]);
  const pending = ref(new Set<string>());
  /** A privileged session is open: privileged actions won't ask for the password again. */
  const sessionActive = ref(false);
  /** Entry whose output is open: it stays until the output is closed. */
  const pinned = ref<number | null>(null);
  /** Auto-dismiss deadline (epoch ms) per finished entry, used for the countdown bar. */
  const deadlines = ref<Record<number, number>>({});
  const timers = new Map<number, ReturnType<typeof setTimeout>>();
  let nextId = 1;

  function clearTimer(id: number): void {
    clearTimeout(timers.get(id));
    timers.delete(id);
    const { [id]: _, ...rest } = deadlines.value;
    deadlines.value = rest;
  }

  function scheduleDismiss(id: number): void {
    clearTimer(id);
    deadlines.value = { ...deadlines.value, [id]: Date.now() + AUTO_DISMISS_MS };
    timers.set(
      id,
      setTimeout(() => {
        clearTimer(id);
        // Being read: removed only after the output is closed (and another 30 s).
        if (pinned.value !== id) dismiss(id);
      }, AUTO_DISMISS_MS),
    );
  }

  /** Opens/closes the output of an entry; closing restarts its 30 s countdown. */
  function togglePinned(id: number): void {
    if (pinned.value === id) {
      pinned.value = null;
      scheduleDismiss(id);
    } else {
      if (pinned.value !== null) scheduleDismiss(pinned.value);
      pinned.value = id;
      clearTimer(id);
    }
  }

  async function refreshSession(): Promise<void> {
    // null = an action is running and holds the session; keep the last known value.
    const active = await invoke<boolean | null>("privileged_session_active").catch(() => null);
    if (active !== null) sessionActive.value = active;
  }

  async function endSession(): Promise<void> {
    await invoke("end_privileged_session");
    sessionActive.value = false;
  }

  function isPending(action: Action): boolean {
    return pending.value.has(actionKey(action));
  }

  /** Runs a valet command and refreshes the state. Returns true when the CLI exited with 0. */
  async function run(action: Action, options: RunOptions): Promise<boolean> {
    const key = actionKey(action);
    if (pending.value.has(key)) return false;
    if (options.confirm && !(await ask(options.confirm, { title: "Valet Linux+", kind: "warning" }))) {
      return false;
    }

    const entry: ActionEntry = { id: nextId++, action, label: options.label, result: null, error: null };
    entries.value = [entry, ...entries.value].slice(0, 20);
    pending.value.add(key);

    try {
      entry.result = await invoke<ActionResult>("run_valet_action", { action });
    } catch (e) {
      entry.error = e instanceof Error ? e.message : String(e);
    } finally {
      pending.value.delete(key);
      // Replace the object so watchers on `entries` see the update.
      entries.value = entries.value.map((e) => (e.id === entry.id ? { ...entry } : e));
      scheduleDismiss(entry.id);
      await Promise.all([useValetStore().refresh(true), refreshSession()]);
    }

    return entry.result?.success ?? false;
  }

  function dismiss(id: number): void {
    clearTimer(id);
    if (pinned.value === id) pinned.value = null;
    entries.value = entries.value.filter((e) => e.id !== id);
  }

  return {
    entries,
    pending,
    sessionActive,
    pinned,
    deadlines,
    isPending,
    run,
    dismiss,
    togglePinned,
    refreshSession,
    endSession,
  };
});
