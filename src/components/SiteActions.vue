<script setup lang="ts">
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { onBeforeUnmount, ref } from "vue";
import { useActionsStore } from "../stores/actions";
import type { Site } from "../types/valet";

const props = defineProps<{ site: Site; phpVersions: string[] }>();
const emit = defineEmits<{ error: [message: string] }>();

const actions = useActionsStore();
const open = ref(false);
const trigger = ref<HTMLElement | null>(null);
const menu = ref<HTMLElement | null>(null);
// The table scrolls horizontally, which would clip an absolute menu: render it fixed in <body>.
const position = ref<Record<string, string>>({});

function onDocumentClick(event: MouseEvent): void {
  const target = event.target as Node;
  if (!menu.value?.contains(target) && !trigger.value?.contains(target)) close();
}

function toggle(): void {
  if (open.value) return close();

  const rect = trigger.value!.getBoundingClientRect();
  const right = `${window.innerWidth - rect.right}px`;
  position.value =
    rect.bottom > window.innerHeight * 0.6
      ? { right, bottom: `${window.innerHeight - rect.top + 4}px` }
      : { right, top: `${rect.bottom + 4}px` };

  open.value = true;
  document.addEventListener("click", onDocumentClick);
  window.addEventListener("resize", close);
  window.addEventListener("scroll", close, true);
}

function close(): void {
  open.value = false;
  document.removeEventListener("click", onDocumentClick);
  window.removeEventListener("resize", close);
  window.removeEventListener("scroll", close, true);
}

onBeforeUnmount(close);

async function guard(task: () => Promise<unknown>): Promise<void> {
  close();
  try {
    await task();
  } catch (e) {
    emit("error", String(e));
  }
}

const browse = () => guard(() => openUrl(props.site.url));
const reveal = () => guard(() => revealItemInDir(props.site.path ?? ""));
const secure = () =>
  guard(() =>
    props.site.secure
      ? actions.run(
          { kind: "unsecure", site: props.site.name },
          { label: `Unsecuring ${props.site.name}`, confirm: `Remove the TLS certificate of ${props.site.domain}?` },
        )
      : actions.run({ kind: "secure", site: props.site.name }, { label: `Securing ${props.site.name}` }),
  );
const isolate = (php: string) =>
  guard(() =>
    actions.run({ kind: "isolate", site: props.site.name, php }, { label: `Isolating ${props.site.name} to PHP ${php}` }),
  );
const unisolate = () =>
  guard(() => actions.run({ kind: "unisolate", site: props.site.name }, { label: `Unisolating ${props.site.name}` }));
const unlink = () =>
  guard(() =>
    actions.run(
      { kind: "unlink", name: props.site.name },
      { label: `Unlinking ${props.site.name}`, confirm: `Remove the ${props.site.name} link? The folder is kept.` },
    ),
  );

const item =
  "flex w-full items-center justify-between gap-4 px-3 py-1.5 text-left hover:bg-zinc-100 disabled:opacity-40 dark:hover:bg-zinc-800";
</script>

<template>
  <div class="inline-flex items-center gap-1">
    <button
      type="button"
      class="rounded px-2 py-1 text-xs font-medium whitespace-nowrap text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
      :title="`Open ${site.url}`"
      @click="browse"
    >
      Open ↗
    </button>
    <button
      ref="trigger"
      type="button"
      class="rounded px-2 py-1 text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
      aria-label="More actions"
      :aria-expanded="open"
      @click.stop="toggle"
    >
      ⋯
    </button>

    <Teleport to="body">
    <div
      v-if="open"
      ref="menu"
      :style="position"
      class="fixed z-50 max-h-[70vh] w-56 overflow-y-auto rounded-md border border-zinc-200 bg-white py-1 text-left text-sm shadow-lg dark:border-zinc-700 dark:bg-zinc-900"
      role="menu"
    >
      <button v-if="site.path" type="button" :class="item" role="menuitem" @click="reveal">Show folder</button>
      <button type="button" :class="item" role="menuitem" @click="secure">
        {{ site.secure ? "Unsecure (HTTP)" : "Secure (HTTPS)" }}
      </button>

      <template v-if="site.type !== 'proxy'">
        <div class="my-1 border-t border-zinc-100 dark:border-zinc-800" />
        <p class="px-3 pt-1 pb-0.5 text-xs text-zinc-500">Isolate PHP</p>
        <button
          v-for="php in phpVersions"
          :key="php"
          type="button"
          :class="item"
          role="menuitem"
          :disabled="site.isolated_php === php"
          @click="isolate(php)"
        >
          <span class="font-mono text-xs">PHP {{ php }}</span>
          <span v-if="site.isolated_php === php" class="text-xs text-zinc-500">current</span>
        </button>
        <button v-if="site.isolated_php" type="button" :class="item" role="menuitem" @click="unisolate">
          Use global PHP
        </button>
      </template>

      <template v-if="site.type === 'linked'">
        <div class="my-1 border-t border-zinc-100 dark:border-zinc-800" />
        <button type="button" :class="[item, 'text-red-700 dark:text-red-400']" role="menuitem" @click="unlink">
          Unlink
        </button>
      </template>
    </div>
    </Teleport>
  </div>
</template>
