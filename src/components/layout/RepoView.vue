<script setup lang="ts">
import { computed } from 'vue'
import { PanelLeftClose, PanelLeftOpen } from '@lucide/vue'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import Toolbar from './Toolbar.vue'
import StatusBar from './StatusBar.vue'
import Sidebar from '@/components/sidebar/Sidebar.vue'
import CommitGraph from '@/components/graph/CommitGraph.vue'
import DiffView from '@/components/diff/DiffView.vue'
import BlameView from '@/components/diff/BlameView.vue'
import DetailPanel from '@/components/commit/DetailPanel.vue'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'

const props = defineProps<{ repo: RepoStore }>()
const repo = props.repo
const ui = useUiStore()

const showSidebar = computed(() => !ui.prefs.sidebarCollapsed && !repo.diffMaximized)
const showDetail = computed(() => !repo.diffMaximized)
const diff = computed(() => repo.diffTarget)

function onLayout(sizes: number[]) {
  if (showSidebar.value && showDetail.value && sizes.length === 3) {
    ui.prefs.sidebarSize = Math.round(sizes[0] * 10) / 10
    ui.prefs.detailSize = Math.round(sizes[2] * 10) / 10
  }
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <Toolbar :repo="repo" />
    <div v-if="repo.summary.bare" class="shrink-0 border-b bg-warning/10 px-3 py-1.5 text-center text-[12px] text-muted-foreground">
      Repositório bare (sem working directory): use uma worktree para trabalhar com arquivos. O histórico pode ser visto abaixo.
    </div>
    <div class="relative min-h-0 flex-1">
      <ResizablePanelGroup direction="horizontal" class="h-full" @layout="onLayout">
        <template v-if="showSidebar">
          <ResizablePanel id="sidebar" :order="1" :default-size="ui.prefs.sidebarSize" :min-size="13" :max-size="40">
            <Sidebar :repo="repo" />
          </ResizablePanel>
          <ResizableHandle />
        </template>
        <ResizablePanel id="center" :order="2" :min-size="30">
          <DiffView v-if="diff && diff.kind !== 'blame'" :repo="repo" :target="diff" />
          <BlameView v-else-if="diff && diff.kind === 'blame'" :repo="repo" :path="diff.path" :rev="diff.rev" />
          <CommitGraph v-else :repo="repo" />
        </ResizablePanel>
        <template v-if="showDetail">
          <ResizableHandle />
          <ResizablePanel id="detail" :order="3" :default-size="ui.prefs.detailSize" :min-size="20" :max-size="50">
            <DetailPanel :repo="repo" />
          </ResizablePanel>
        </template>
      </ResizablePanelGroup>
      <button
        class="absolute bottom-2 left-2 z-20 rounded-md border bg-popover p-1 text-muted-foreground shadow hover:text-foreground"
        :title="ui.prefs.sidebarCollapsed ? 'Mostrar barra lateral' : 'Ocultar barra lateral'"
        @click="ui.prefs.sidebarCollapsed = !ui.prefs.sidebarCollapsed"
      >
        <component :is="ui.prefs.sidebarCollapsed ? PanelLeftOpen : PanelLeftClose" class="size-4" />
      </button>
    </div>
    <StatusBar :repo="repo" />
  </div>
</template>
