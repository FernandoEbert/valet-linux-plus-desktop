<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { onMounted, ref } from "vue";
import { storeToRefs } from "pinia";
import ToggleSwitch from "../components/ToggleSwitch.vue";
import { useUpdaterStore } from "../stores/updater";

interface AppSettings {
  notify_service_down: boolean;
  close_to_tray: boolean;
  tray_available: boolean;
}

const updater = useUpdaterStore();
const { currentVersion, kind, update, checking, lastChecked, supported } = storeToRefs(updater);
const updateError = storeToRefs(updater).error;

const kindLabel: Record<string, string> = { appimage: "AppImage", deb: ".deb package", rpm: ".rpm package" };

const settings = ref<AppSettings | null>(null);
const autostart = ref(false);
const error = ref<string | null>(null);

onMounted(async () => {
  try {
    [settings.value, autostart.value] = await Promise.all([invoke<AppSettings>("get_settings"), isEnabled()]);
  } catch (e) {
    error.value = String(e);
  }
});

async function save(patch: Partial<AppSettings>): Promise<void> {
  if (!settings.value) return;
  const { tray_available: _, ...current } = settings.value;
  try {
    settings.value = await invoke<AppSettings>("set_settings", { settings: { ...current, ...patch } });
    error.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

async function setAutostart(on: boolean): Promise<void> {
  try {
    await (on ? enable() : disable());
    autostart.value = await isEnabled();
    error.value = null;
  } catch (e) {
    autostart.value = !on;
    error.value = String(e);
  }
}
</script>

<template>
  <div class="flex max-w-2xl flex-col gap-4">
    <p v-if="error" class="font-mono text-xs text-red-600 dark:text-red-400">{{ error }}</p>

    <section
      v-if="settings"
      class="divide-y divide-zinc-100 rounded-lg border border-zinc-200 bg-white px-4 dark:divide-zinc-800 dark:border-zinc-800 dark:bg-zinc-900"
    >
      <ToggleSwitch
        :model-value="autostart"
        label="Start at login"
        :description="
          settings.tray_available
            ? 'Opens hidden in the tray when you log in.'
            : 'Opens the window when you log in (no tray available on this desktop).'
        "
        @update:model-value="setAutostart"
      />
      <ToggleSwitch
        :model-value="settings.close_to_tray && settings.tray_available"
        label="Keep running in the tray when the window is closed"
        :description="
          settings.tray_available
            ? 'Use Quit in the tray menu to exit.'
            : 'No tray on this desktop (on GNOME, install the AppIndicator extension), so closing the window quits.'
        "
        :disabled="!settings.tray_available"
        @update:model-value="save({ close_to_tray: $event })"
      />
      <ToggleSwitch
        :model-value="settings.notify_service_down"
        label="Notify when a service stops"
        description="Desktop notification when nginx, PHP-FPM, MySQL or another running service goes down."
        @update:model-value="save({ notify_service_down: $event })"
      />
    </section>
    <p v-else-if="!error" class="text-zinc-500">Loading…</p>

    <section class="rounded-lg border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900">
      <h2 class="mb-3 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Updates</h2>
      <div class="flex flex-wrap items-center justify-between gap-4">
        <div class="min-w-0">
          <p>
            Version <span class="font-mono">{{ currentVersion ?? "…" }}</span>
            <span v-if="kind" class="text-zinc-500"> · {{ kindLabel[kind] }}</span>
          </p>
          <p class="mt-0.5 text-xs text-zinc-500">
            <template v-if="!supported">Updates are only available in the AppImage, .deb and .rpm builds.</template>
            <template v-else-if="checking">Checking…</template>
            <template v-else-if="update">Version {{ update.version }} is available (see the banner above).</template>
            <template v-else-if="lastChecked">You're up to date (checked at {{ lastChecked.toLocaleTimeString() }}).</template>
            <template v-else>Checked automatically when the app starts.</template>
          </p>
          <p v-if="updateError" class="mt-1 font-mono text-xs text-red-600 dark:text-red-400">{{ updateError }}</p>
        </div>
        <button
          type="button"
          class="rounded-md border border-zinc-300 bg-white px-3 py-1.5 text-sm font-medium whitespace-nowrap hover:bg-zinc-50 disabled:opacity-50 dark:border-zinc-700 dark:bg-zinc-900 dark:hover:bg-zinc-800"
          :disabled="!supported || checking"
          @click="updater.checkForUpdates()"
        >
          Check for updates
        </button>
      </div>
    </section>
  </div>
</template>
