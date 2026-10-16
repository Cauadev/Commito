<script setup lang="ts">
import { ref } from 'vue'
import { FolderGit2, House, Plus, X } from '@lucide/vue'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from '@/components/ui/context-menu'
import { copyText } from '@/features/actions'
import { api, toAppError } from '@/services/api'
import { useAppStore } from '@/stores/app'
import { findRepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { ResolvedRepo } from '@/types/git'

const app = useAppStore()
const ui = useUiStore()
const dragFrom = ref<number | null>(null)
const menuTab = ref<ResolvedRepo | null>(null)

function onDrop(i: number) {
  if (dragFrom.value != null) app.moveTab(dragFrom.value, i)
  dragFrom.value = null
}
async function closeOthers(keep: string) {
  for (const t of [...app.tabs]) if (t.path !== keep) await app.closeTab(t.path)
}
const changes = (t: ResolvedRepo) => findRepoStore(t.path)?.changeCount ?? 0
</script>

<template>
  <div class="flex h-9 shrink-0 items-stretch border-b bg-chrome select-none">
    <button
      class="flex w-10 items-center justify-center border-r text-muted-foreground hover:bg-muted hover:text-foreground"
      :class="!app.activePath ? 'bg-background text-foreground' : ''"
      title="Início"
      @click="app.showHome()"
    >
      <House class="size-4" />
    </button>
    <ContextMenu>
      <ContextMenuTrigger as-child>
        <div class="flex min-w-0 flex-1 items-stretch overflow-x-auto">
          <div
            v-for="(t, i) in app.tabs"
            :key="t.path"
            draggable="true"
            class="group relative flex max-w-56 min-w-28 cursor-default items-center gap-1.5 border-r px-3 text-[12.5px]"
            :class="t.path === app.activePath ? 'bg-background text-foreground' : 'text-muted-foreground hover:bg-muted/60'"
            :title="t.path"
            @click="app.activate(t.path)"
            @auxclick.middle="app.closeTab(t.path)"
            @dragstart="dragFrom = i"
            @dragover.prevent
            @drop="onDrop(i)"
            @contextmenu="menuTab = t"
          >
            <span v-if="t.path === app.activePath" class="absolute inset-x-0 top-0 h-0.5 bg-primary" />
            <FolderGit2 class="size-3.5 shrink-0" :class="t.linkedWorktree ? 'text-warning' : ''" />
            <span class="truncate">{{ t.name }}</span>
            <span v-if="changes(t)" class="rounded-full bg-muted px-1.5 text-[10px] tabular-nums">{{ changes(t) }}</span>
            <button class="ml-auto rounded p-0.5 opacity-0 group-hover:opacity-100 hover:bg-muted" :class="t.path === app.activePath ? 'opacity-70' : ''" title="Fechar (Ctrl+W)" @click.stop="app.closeTab(t.path)">
              <X class="size-3.5" />
            </button>
          </div>
          <button class="flex w-9 items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground" title="Abrir outro repositório" @click="app.showHome()">
            <Plus class="size-4" />
          </button>
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent class="w-56">
        <template v-if="menuTab">
        <ContextMenuItem @select="app.closeTab(menuTab!.path)">Fechar aba</ContextMenuItem>
        <ContextMenuItem :disabled="app.tabs.length < 2" @select="closeOthers(menuTab!.path)">Fechar as outras</ContextMenuItem>
        <ContextMenuItem :disabled="!app.closedTabs.length" @select="app.reopenClosed()">Reabrir aba fechada (Ctrl+Shift+T)</ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem @select="api.openInExplorer(menuTab!.path).catch((e) => ui.error(toAppError(e)))">Mostrar no Explorer</ContextMenuItem>
        <ContextMenuItem @select="copyText(menuTab!.path, 'Caminho copiado.')">Copiar caminho</ContextMenuItem>
              </template>
        <ContextMenuItem v-else disabled>Nenhuma ação aqui</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  </div>
</template>
