<script setup lang="ts">
import { computed } from 'vue'
import { ArrowDown, ArrowUp, CircleAlert, FileDiff, GitBranch, LoaderCircle, Wifi, WifiOff, X, Link2Off } from '@lucide/vue'
import { api } from '@/services/api'
import FooterActions from './FooterActions.vue'
import { relativeTime } from '@/lib/format'
import { useAppStore } from '@/stores/app'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'

const props = defineProps<{ repo: RepoStore }>()
const repo = props.repo
const app = useAppStore()
const ui = useUiStore()
const head = computed(() => repo.status?.head)
const ops = computed(() => Object.values(ui.ops).filter((o) => !o.repo || o.repo === repo.path))
</script>

<template>
  <div class="flex h-7 shrink-0 items-center gap-3 border-t bg-chrome px-3 text-[11.5px] text-muted-foreground select-none">
    <span class="flex items-center gap-1" :title="app.online ? 'Conectado à rede' : 'Sem rede: operações locais continuam disponíveis'">
      <component :is="app.online ? Wifi : WifiOff" class="size-3.5" :class="app.online ? 'text-success' : 'text-warning'" />
      {{ app.online ? 'Online' : 'Offline' }}
    </span>
    <span class="flex items-center gap-1 text-foreground/90">
      <GitBranch class="size-3.5" /> {{ repo.currentBranch ?? (head?.detached ? 'HEAD destacado' : '—') }}
    </span>
    <template v-if="head?.upstream">
      <span class="flex items-center gap-2" :title="`Comparado a ${head.upstream}${repo.lastFetch ? '' : ' (desde o último fetch)'}`">
        <span class="flex items-center" :class="head.ahead ? 'text-info' : ''"><ArrowUp class="size-3.5" />{{ head.ahead ?? 0 }}</span>
        <span class="flex items-center" :class="head.behind ? 'text-warning' : ''"><ArrowDown class="size-3.5" />{{ head.behind ?? 0 }}</span>
        <span class="max-w-40 truncate">{{ head.upstream }}</span>
      </span>
    </template>
    <span v-else-if="repo.currentBranch" class="flex items-center gap-1" title="Sem upstream: ahead/behind indisponível"><Link2Off class="size-3.5" /> sem upstream</span>
    <span class="flex items-center gap-1"><FileDiff class="size-3.5" /> {{ repo.changeCount }} {{ repo.changeCount === 1 ? 'arquivo alterado' : 'arquivos alterados' }}</span>
    <span v-if="repo.status?.operation" class="flex items-center gap-1 text-warning"><CircleAlert class="size-3.5" /> {{ repo.status.operation.kind.replace('_', '-') }} em andamento</span>

    <div class="ml-auto flex min-w-0 items-center gap-3">
      <span v-if="repo.busy && !ops.length" class="flex items-center gap-1.5 text-foreground"><LoaderCircle class="size-3.5 animate-spin" /> {{ repo.busy }}…</span>
      <span v-for="o in ops" :key="o.opId" class="flex min-w-0 items-center gap-2 text-foreground">
        <LoaderCircle class="size-3.5 shrink-0 animate-spin text-primary" />
        <span class="shrink-0">{{ o.label }}</span>
        <span v-if="o.percent != null" class="h-1.5 w-24 overflow-hidden rounded-full bg-muted"><span class="block h-full bg-primary transition-all" :style="{ width: o.percent + '%' }" /></span>
        <span class="max-w-60 truncate text-muted-foreground">{{ o.phase || o.line }}</span>
        <button v-if="o.cancellable" class="rounded p-0.5 hover:bg-muted" title="Cancelar" @click="api.cancelOperation(o.opId)"><X class="size-3.5" /></button>
      </span>
      <span v-if="repo.lastFetch" class="shrink-0">sincronizado {{ relativeTime(repo.lastFetch / 1000) }}</span>
      <button
        v-if="Math.abs(ui.prefs.zoom - 1) > 0.001"
        class="shrink-0 rounded px-1 tabular-nums hover:bg-muted hover:text-foreground"
        title="Zoom da interface (Ctrl + / Ctrl − / Ctrl+0). Clique para voltar a 100%."
        @click="ui.stepZoom(0)"
      >{{ Math.round(ui.prefs.zoom * 100) }}%</button>
      <span class="shrink-0 text-faint">{{ app.info?.gitVersion?.replace('git version ', 'git ') }}</span>
      <FooterActions />
    </div>
  </div>
</template>
