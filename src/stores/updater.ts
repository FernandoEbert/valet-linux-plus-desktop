import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { defineStore } from "pinia";
import { computed, ref, shallowRef } from "vue";

export type InstallKind = "appimage" | "deb" | "rpm";

const RELEASES_URL = "https://github.com/FernandoEbert/valet-linux-plus-desktop/releases";

export const useUpdaterStore = defineStore("updater", () => {
  const currentVersion = ref<string | null>(null);
  /** null = dev build / plain binary: nothing to update in place. */
  const kind = ref<InstallKind | null>(null);
  const update = shallowRef<Update | null>(null);
  const checking = ref(false);
  const installing = ref(false);
  /** 0–100 while downloading, null when the size is unknown. */
  const progress = ref<number | null>(null);
  const error = ref<string | null>(null);
  const lastChecked = ref<Date | null>(null);
  const dismissed = ref(false);

  const supported = computed(() => kind.value !== null);
  const releaseUrl = computed(() => (update.value ? `${RELEASES_URL}/tag/v${update.value.version}` : RELEASES_URL));
  const showBanner = computed(() => update.value !== null && !dismissed.value);

  async function init(): Promise<void> {
    [currentVersion.value, kind.value] = await Promise.all([
      getVersion(),
      invoke<InstallKind | null>("install_kind"),
    ]);
  }

  /** `silent`: background check on startup, errors are not shown (offline, GitHub down…). */
  async function checkForUpdates(silent = false): Promise<void> {
    if (checking.value || !supported.value) return;
    checking.value = true;
    if (!silent) error.value = null;
    try {
      update.value = await check();
      dismissed.value = false;
      lastChecked.value = new Date();
    } catch (e) {
      if (!silent) error.value = String(e);
    } finally {
      checking.value = false;
    }
  }

  /** AppImage: replaces the file. .deb/.rpm: installs the package through pkexec. Then restarts. */
  async function installAndRestart(): Promise<void> {
    if (!update.value || installing.value) return;
    installing.value = true;
    error.value = null;
    let total = 0;
    let received = 0;
    try {
      await update.value.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
          progress.value = total ? 0 : null;
        } else if (event.event === "Progress" && total) {
          received += event.data.chunkLength;
          progress.value = Math.min(100, Math.round((received / total) * 100));
        }
      });
      await relaunch();
    } catch (e) {
      error.value = String(e);
    } finally {
      installing.value = false;
      progress.value = null;
    }
  }

  return {
    currentVersion,
    kind,
    update,
    checking,
    installing,
    progress,
    error,
    lastChecked,
    dismissed,
    supported,
    releaseUrl,
    showBanner,
    init,
    checkForUpdates,
    installAndRestart,
  };
});
