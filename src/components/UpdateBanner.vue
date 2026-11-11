<script setup lang="ts">
import { openUrl } from "@tauri-apps/plugin-opener";
import { storeToRefs } from "pinia";
import { useUpdaterStore } from "../stores/updater";

const updater = useUpdaterStore();
const { update, installing, progress, error, releaseUrl, kind } = storeToRefs(updater);
</script>

<template>
  <div
    v-if="update"
    class="mb-4 flex flex-wrap items-center gap-3 rounded-md border border-sky-300/70 bg-sky-50 px-4 py-2.5 text-sm text-sky-900 dark:border-sky-900/60 dark:bg-sky-950/30 dark:text-sky-200"
    role="status"
  >
    <p class="min-w-0 flex-1">
      <span class="font-medium">Valet Linux+ {{ update.version }}</span> is available.
      <template v-if="installing">
        {{ progress === null ? "Downloading…" : `Downloading… ${progress}%` }}
        <template v-if="kind !== 'appimage'"> Your password will be asked to install the package.</template>
      </template>
      <span v-if="error" class="block font-mono text-xs text-red-700 dark:text-red-400">{{ error }}</span>
    </p>
    <button
      type="button"
      class="text-xs underline underline-offset-2"
      @click="openUrl(releaseUrl)"
    >
      What's new
    </button>
    <button
      type="button"
      class="rounded-md bg-sky-900 px-3 py-1 text-xs font-medium text-white hover:bg-sky-800 disabled:opacity-60 dark:bg-sky-200 dark:text-sky-950 dark:hover:bg-sky-100"
      :disabled="installing"
      @click="updater.installAndRestart()"
    >
      {{ installing ? "Installing…" : "Install and restart" }}
    </button>
    <button
      v-if="!installing"
      type="button"
      class="text-sky-700 hover:text-sky-950 dark:text-sky-300 dark:hover:text-white"
      aria-label="Dismiss"
      @click="updater.dismissed = true"
    >
      ×
    </button>
  </div>
</template>
