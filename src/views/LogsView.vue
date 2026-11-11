<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import type { LogFile, LogTail } from "../types/valet";

const LINE_OPTIONS = [100, 200, 500, 1000, 2000];

const files = ref<LogFile[]>([]);
const selected = ref<string | null>(null);
const lines = ref(200);
const follow = ref(true);
const tail = ref<LogTail | null>(null);
const error = ref<string | null>(null);
const output = ref<HTMLElement | null>(null);
let timer: ReturnType<typeof setInterval> | null = null;
let reading = false;

const options = computed(() => files.value.filter((f) => f.size > 0 || f.name === "nginx-error.log" || f.name === selected.value));

function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
}

async function loadFiles(): Promise<void> {
  try {
    files.value = await invoke<LogFile[]>("get_logs");
    selected.value ??= files.value[0]?.name ?? null;
  } catch (e) {
    error.value = String(e);
  }
}

async function read(): Promise<void> {
  if (!selected.value || reading) return;
  reading = true;
  const el = output.value;
  const atBottom = !el || el.scrollHeight - el.scrollTop - el.clientHeight < 24;
  try {
    tail.value = await invoke<LogTail>("read_log", { name: selected.value, lines: lines.value });
    error.value = null;
    if (atBottom) {
      await nextTick();
      output.value?.scrollTo({ top: output.value.scrollHeight });
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    reading = false;
  }
}

watch([selected, lines], () => void read());
watch(
  follow,
  (on) => {
    if (timer) clearInterval(timer);
    timer = on ? setInterval(() => document.visibilityState === "visible" && void read(), 2000) : null;
  },
  { immediate: true },
);

onMounted(async () => {
  await loadFiles();
  await read();
});
onUnmounted(() => timer && clearInterval(timer));
</script>

<template>
  <div class="flex h-full flex-col gap-3">
    <div class="flex flex-wrap items-center gap-3">
      <select
        v-model="selected"
        class="min-w-64 rounded-md border border-zinc-300 bg-white px-2 py-1.5 font-mono text-[13px] dark:border-zinc-700 dark:bg-zinc-900"
        @focus="loadFiles"
      >
        <option v-for="file in options" :key="file.name" :value="file.name">{{ file.name }} ({{ size(file.size) }})</option>
      </select>
      <select v-model.number="lines" class="rounded-md border border-zinc-300 bg-white px-2 py-1.5 text-sm dark:border-zinc-700 dark:bg-zinc-900">
        <option v-for="n in LINE_OPTIONS" :key="n" :value="n">last {{ n }} lines</option>
      </select>
      <label class="flex items-center gap-2 text-sm text-zinc-600 dark:text-zinc-400">
        <input v-model="follow" type="checkbox" class="accent-zinc-900 dark:accent-zinc-100" /> Follow
      </label>
      <span v-if="tail" class="ml-auto text-xs text-zinc-500">{{ size(tail.size) }}</span>
    </div>

    <p v-if="error" class="font-mono text-xs text-red-600 dark:text-red-400">{{ error }}</p>

    <pre
      ref="output"
      class="min-h-80 flex-1 overflow-auto rounded-lg border border-zinc-200 bg-white p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap dark:border-zinc-800 dark:bg-zinc-900"
    ><template v-if="tail?.truncated"><span class="text-zinc-400">… earlier lines omitted&#10;</span></template>{{ tail?.lines.join("\n") || (selected ? "(empty)" : "No logs in ~/.config/valet/Log") }}</pre>
  </div>
</template>
