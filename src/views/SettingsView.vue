<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { onMounted, ref } from "vue";
import ToggleSwitch from "../components/ToggleSwitch.vue";

interface AppSettings {
  notify_service_down: boolean;
  close_to_tray: boolean;
  tray_available: boolean;
}

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
  </div>
</template>
