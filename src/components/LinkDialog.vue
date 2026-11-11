<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useActionsStore } from "../stores/actions";

const props = defineProps<{ path: string | null; domain: string }>();
const emit = defineEmits<{ close: [] }>();

const actions = useActionsStore();
const name = ref("");
const input = ref<HTMLInputElement | null>(null);

// Same rule the backend enforces.
const valid = computed(() => /^[A-Za-z0-9][A-Za-z0-9._-]{0,252}$/.test(name.value));

watch(
  () => props.path,
  async (path) => {
    if (!path) return;
    name.value = (path.split("/").filter(Boolean).pop() ?? "").replace(/[^A-Za-z0-9._-]/g, "-");
    await nextTick();
    input.value?.select();
  },
  { immediate: true },
);

async function submit(): Promise<void> {
  if (!props.path || !valid.value) return;
  emit("close");
  await actions.run({ kind: "link", path: props.path, name: name.value }, { label: `Linking ${name.value}` });
}
</script>

<template>
  <div v-if="path" class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4" @click.self="emit('close')">
    <form
      class="w-full max-w-md rounded-lg border border-zinc-200 bg-white p-5 shadow-xl dark:border-zinc-700 dark:bg-zinc-900"
      @submit.prevent="submit"
      @keydown.esc="emit('close')"
    >
      <h2 class="text-base font-semibold">Link folder</h2>
      <p class="mt-1 truncate font-mono text-xs text-zinc-500" :title="path">{{ path }}</p>

      <label class="mt-4 block text-xs text-zinc-500" for="link-name">Site name</label>
      <div class="mt-1 flex items-center rounded-md border border-zinc-300 focus-within:border-zinc-500 dark:border-zinc-700">
        <input
          id="link-name"
          ref="input"
          v-model.trim="name"
          class="min-w-0 flex-1 bg-transparent px-3 py-1.5 font-mono text-sm outline-none"
          autocomplete="off"
          spellcheck="false"
        />
        <span class="pr-3 font-mono text-sm text-zinc-400">.{{ domain }}</span>
      </div>
      <p v-if="name && !valid" class="mt-1 text-xs text-red-600 dark:text-red-400">
        Letters, numbers, dots, dashes and underscores only.
      </p>

      <div class="mt-5 flex justify-end gap-2">
        <button
          type="button"
          class="rounded-md border border-zinc-300 px-3 py-1.5 text-sm hover:bg-zinc-50 dark:border-zinc-700 dark:hover:bg-zinc-800"
          @click="emit('close')"
        >
          Cancel
        </button>
        <button
          type="submit"
          class="rounded-md bg-zinc-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-zinc-700 disabled:opacity-50 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-zinc-300"
          :disabled="!valid"
        >
          Link
        </button>
      </div>
    </form>
  </div>
</template>
