<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useElementSize, useEventListener } from '@vueuse/core'
import { ChevronDown, ChevronUp, Columns3, LoaderCircle, Pencil, Plus, Minus, Search, X } from '@lucide/vue'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@/components/ui/context-menu'
import {
  DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Button } from '@/components/ui/button'
import NativeSelect from '@/components/common/NativeSelect.vue'
import GraphCell from './GraphCell.vue'
import RefBadges from './RefBadges.vue'
import CommitMenuItems from '@/components/menus/CommitMenuItems.vue'
import RefMenuItems from '@/components/menus/RefMenuItems.vue'
import { useActions } from '@/features/actions'
import { absoluteTime, hashHue, initials, laneColor, relativeTime, shortSha } from '@/lib/format'
import { api, toAppError } from '@/services/api'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { GitRef, GraphRow, GraphScope } from '@/types/git'
import { t } from '@/i18n'

const props = defineProps<{ repo: RepoStore }>()
const repo = props.repo
const ui = useUiStore()
const actions = useActions(repo)

const ROW_H = 28
const LANE_W = 16
const OVERSCAN = 12

const scroller = ref<HTMLElement | null>(null)
const scrollTop = ref(0)
const { height: viewH } = useElementSize(scroller)

const cols = computed(() => ui.prefs.graphColumns)
const graphW = computed(() => Math.min(Math.max((repo.graph?.maxLanes ?? 1) * LANE_W + 14, 44), 360))

const rows = computed(() => repo.rows)
const start = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_H) - OVERSCAN))
const end = computed(() => Math.min(rows.value.length, Math.ceil((scrollTop.value + viewH.value) / ROW_H) + OVERSCAN))
const visible = computed(() => {
  const out: { row: GraphRow; index: number }[] = []
  for (let i = start.value; i < end.value; i++) out.push({ row: rows.value[i], index: i })
  return out
})

const selectedSet = computed(() => {
  const s = repo.selection
  if (s.kind === 'commits') return new Set(s.hashes)
  if (s.kind === 'stash') return new Set([s.hash])
  return new Set(['WIP'])
})
const matchSet = computed(() => new Set(repo.filterMatches))
const filtering = computed(() => repo.filter.trim().length > 0)

function onScroll() {
  scrollTop.value = scroller.value?.scrollTop ?? 0
  if (repo.graph?.hasMore && end.value > rows.value.length - 200) void repo.loadMore()
}

function scrollToIndex(i: number, center = false) {
  const el = scroller.value
  if (!el) return
  const top = i * ROW_H
  if (center) el.scrollTop = Math.max(0, top - el.clientHeight / 2 + ROW_H / 2)
  else if (top < el.scrollTop) el.scrollTop = top
  else if (top + ROW_H > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW_H - el.clientHeight
}

function onRowClick(row: GraphRow, e: MouseEvent) {
  if (row.kind === 'wip') return repo.selectWip()
  repo.selectCommit(row.hash, e.ctrlKey || e.metaKey ? 'toggle' : e.shiftKey ? 'range' : 'replace')
  scroller.value?.focus({ preventScroll: true })
}

function onRowDblClick(row: GraphRow) {
  if (row.kind !== 'commit') return
  const refs = repo.refsByTarget.get(row.hash) ?? []
  const local = refs.find((r) => r.kind === 'local' && !r.isHead)
  if (local) void actions.checkoutRef(local)
}

// keyboard navigation
function currentIndex(): number {
  const s = repo.selection
  if (s.kind === 'wip') return repo.rowIndex.get('WIP') ?? 0
  const h = s.kind === 'commits' ? s.anchor : s.hash
  return repo.rowIndex.get(h) ?? 0
}
function onKey(e: KeyboardEvent) {
  if (!rows.value.length) return
  let i = currentIndex()
  const page = Math.max(1, Math.floor(viewH.value / ROW_H) - 1)
  if (e.key === 'ArrowDown') i++
  else if (e.key === 'ArrowUp') i--
  else if (e.key === 'PageDown') i += page
  else if (e.key === 'PageUp') i -= page
  else if (e.key === 'Home') i = 0
  else if (e.key === 'End') i = rows.value.length - 1
  else return
  e.preventDefault()
  i = Math.max(0, Math.min(rows.value.length - 1, i))
  const row = rows.value[i]
  if (row.kind === 'wip') repo.selectWip()
  else repo.selectCommit(row.hash, e.shiftKey ? 'range' : 'replace')
  scrollToIndex(i)
}

// context menu target
const menu = ref<{ type: 'commit'; row: GraphRow } | { type: 'ref'; ref: GitRef } | { type: 'wip' } | { type: 'stash'; row: GraphRow } | null>(null)
let pendingRef: GitRef | null = null
function onBadgeCtx(r: GitRef) {
  pendingRef = r
}
function onRowCtx(row: GraphRow) {
  if (pendingRef) {
    menu.value = { type: 'ref', ref: pendingRef }
    pendingRef = null
    return
  }
  if (row.kind === 'wip') {
    repo.selectWip()
    menu.value = { type: 'wip' }
    return
  }
  if (row.kind === 'stash') {
    repo.selectCommit(row.hash)
    menu.value = { type: 'stash', row }
    return
  }
  if (!selectedSet.value.has(row.hash)) repo.selectCommit(row.hash)
  menu.value = { type: 'commit', row }
}

const stashOf = (row: GraphRow) => repo.stashes.find((s) => s.hash === row.hash)

async function compare(base: string, target: string) {
  try {
    const files = await api.compareRefs(repo.path, base, target)
    repo.compare = { base, target, files }
  } catch (e) {
    ui.error(toAppError(e))
  }
}

// filter navigation
function gotoMatch(delta: number) {
  const m = repo.filterMatches
  if (!m.length) return
  repo.filterIndex = (repo.filterIndex + delta + m.length) % m.length
  const idx = m[repo.filterIndex]
  const row = rows.value[idx]
  repo.selectCommit(row.hash)
  scrollToIndex(idx, true)
}
watch(() => repo.filter, () => {
  repo.filterIndex = -1
})

const searching = ref(false)
async function deepSearch() {
  const q = repo.filter.trim()
  if (!q) return
  searching.value = true
  try {
    const hashes = await api.searchCommits(repo.path, q, repo.scope)
    if (!hashes.length) {
      ui.toast('info', 'Nenhum commit encontrado no histórico.', `Busca por mensagem, autor ou SHA: "${q}"`)
      return
    }
    const ok = await repo.ensureLoaded(hashes[0])
    if (ok) {
      const idx = repo.rowIndex.get(hashes[0])!
      repo.selectCommit(hashes[0])
      await nextTick()
      scrollToIndex(idx, true)
      ui.toast('info', `${hashes.length} commit(s) encontrados no histórico completo.`, 'Use Enter para navegar entre os resultados carregados.')
    } else {
      ui.toast('warning', 'Commit encontrado, mas fora do intervalo carregado.', shortSha(hashes[0]))
    }
  } catch (e) {
    ui.error(toAppError(e))
  } finally {
    searching.value = false
  }
}

const scopeOptions = computed(() => (['all', 'current', 'local', 'remote'] as GraphScope[]).map((s) => ({ value: s, label: t(`graph.scope.${s}`) })))
const scopeModel = computed({ get: () => repo.scope, set: (v: string) => void repo.setScope(v as GraphScope) })

// reveal a commit when selection changes from outside (sidebar, search)
watch(
  () => repo.selection,
  (s) => {
    if (s.kind === 'commits' && s.hashes.length === 1) {
      const i = repo.rowIndex.get(s.hashes[0])
      if (i != null) scrollToIndex(i)
    }
  },
)

// column resize
function startResize(col: 'refs' | 'author' | 'date', e: PointerEvent, dir = 1) {
  const startX = e.clientX
  const startW = cols.value[col]
  const move = (ev: PointerEvent) => {
    cols.value[col] = Math.max(60, Math.min(420, startW + dir * (ev.clientX - startX)))
  }
  const up = () => {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', up)
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', up)
}

const filterInput = ref<HTMLInputElement | null>(null)
useEventListener(window, 'keydown', (e: KeyboardEvent) => {
  if (e.ctrlKey && e.altKey && e.key.toLowerCase() === 'f') return
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === 'f') {
    e.preventDefault()
    filterInput.value?.focus()
    filterInput.value?.select()
  }
})

onMounted(() => {
  scrollTop.value = scroller.value?.scrollTop ?? 0
})

const wipSummary = computed(() => {
  const s = repo.status
  if (!s) return { mod: 0, add: 0, del: 0 }
  const all = new Map<string, string>()
  for (const f of [...s.unstaged, ...s.staged, ...s.conflicted]) all.set(f.path, f.status)
  let mod = 0
  let add = 0
  let del = 0
  for (const st of all.values()) {
    if (st === 'added' || st === 'untracked') add++
    else if (st === 'deleted') del++
    else mod++
  }
  return { mod, add, del }
})

const avatarStyle = (name: string) => ({ background: `oklch(0.62 0.12 ${hashHue(name)})` })
</script>

<template>
  <div class="flex h-full min-w-0 flex-col bg-background">
    <!-- graph toolbar -->
    <div class="flex h-9 shrink-0 items-center gap-2 border-b bg-panel px-2">
      <NativeSelect v-model="scopeModel" :options="scopeOptions" class="w-44" />
      <div class="relative flex min-w-0 max-w-md flex-1 items-center">
        <Search class="pointer-events-none absolute left-2 size-3.5 text-muted-foreground" />
        <input
          ref="filterInput"
          v-model="repo.filter"
          class="h-7 w-full rounded-md border border-input bg-transparent pr-24 pl-7 text-[12.5px] outline-none focus:border-ring dark:bg-input/30"
          placeholder="Filtrar commits por mensagem, autor ou SHA (Ctrl+F)"
          @keydown.enter.exact.prevent="gotoMatch(1)"
          @keydown.shift.enter.prevent="gotoMatch(-1)"
          @keydown.esc="repo.filter = ''"
        />
        <div v-if="filtering" class="absolute right-1 flex items-center gap-0.5">
          <span class="px-1 text-[11px] text-muted-foreground tabular-nums">{{ repo.filterMatches.length ? `${Math.max(repo.filterIndex + 1, 0)}/${repo.filterMatches.length}` : '0' }}</span>
          <button class="rounded p-0.5 hover:bg-muted" title="Anterior" @click="gotoMatch(-1)"><ChevronUp class="size-3.5" /></button>
          <button class="rounded p-0.5 hover:bg-muted" title="Próximo" @click="gotoMatch(1)"><ChevronDown class="size-3.5" /></button>
          <button class="rounded p-0.5 hover:bg-muted" title="Limpar" @click="repo.filter = ''"><X class="size-3.5" /></button>
        </div>
      </div>
      <Button v-if="filtering" variant="ghost" size="xs" :disabled="searching" title="Buscar em todo o histórico (não só no que está carregado)" @click="deepSearch">
        <LoaderCircle v-if="searching" class="animate-spin" />
        Buscar no histórico
      </Button>
      <div class="ml-auto flex items-center gap-2 text-[11.5px] text-muted-foreground">
        <LoaderCircle v-if="repo.loading.graph" class="size-3.5 animate-spin" />
        <span class="tabular-nums">{{ repo.graph?.totalLoaded ?? 0 }}{{ repo.graph?.hasMore ? '+' : '' }} commits</span>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button variant="ghost" size="icon-xs" title="Colunas"><Columns3 /></Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuLabel>Colunas</DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuCheckboxItem v-model="cols.showAuthor">{{ t('graph.author') }}</DropdownMenuCheckboxItem>
            <DropdownMenuCheckboxItem v-model="cols.showSha">{{ t('graph.sha') }}</DropdownMenuCheckboxItem>
            <DropdownMenuCheckboxItem v-model="cols.showDate">{{ t('graph.date') }}</DropdownMenuCheckboxItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>

    <!-- column header -->
    <div class="flex h-6 shrink-0 items-center border-b bg-panel text-[10.5px] font-semibold tracking-wider text-muted-foreground uppercase">
      <div class="relative shrink-0 truncate px-2" :style="{ width: cols.refs + 'px' }">
        {{ t('graph.branchTag') }}
        <span class="absolute top-0 right-0 h-full w-1.5 cursor-col-resize hover:bg-primary/40" @pointerdown.prevent="startResize('refs', $event)" />
      </div>
      <div class="shrink-0 px-1" :style="{ width: graphW + 'px' }">{{ t('graph.graph') }}</div>
      <div class="min-w-0 flex-1 px-2">{{ t('graph.message') }}</div>
      <div v-if="cols.showAuthor" class="relative shrink-0 px-2" :style="{ width: cols.author + 'px' }">
        <span class="absolute top-0 left-0 h-full w-1.5 cursor-col-resize hover:bg-primary/40" @pointerdown.prevent="startResize('author', $event, -1)" />
        {{ t('graph.author') }}
      </div>
      <div v-if="cols.showSha" class="shrink-0 px-2" :style="{ width: cols.sha + 'px' }">{{ t('graph.sha') }}</div>
      <div v-if="cols.showDate" class="shrink-0 px-2" :style="{ width: cols.date + 'px' }">{{ t('graph.date') }}</div>
    </div>

    <!-- rows -->
    <ContextMenu>
      <ContextMenuTrigger as-child>
        <div
          ref="scroller"
          tabindex="0"
          class="relative min-h-0 flex-1 overflow-auto outline-none"
          @scroll.passive="onScroll"
          @keydown="onKey"
        >
          <div v-if="!rows.length && !repo.loading.graph" class="flex h-full flex-col items-center justify-center gap-1 text-muted-foreground">
            <span class="text-[13px]">Nenhum commit para exibir.</span>
            <span class="text-[12px]">{{ repo.status?.head.unborn ? 'Este repositório ainda não tem commits: prepare arquivos e faça o primeiro commit.' : 'Tente outro filtro de branches.' }}</span>
          </div>
          <div :style="{ height: rows.length * ROW_H + 'px' }" class="relative min-w-full">
            <div
              v-for="{ row, index } in visible"
              :key="row.hash"
              class="group absolute right-0 left-0 flex items-center text-[12.5px]"
              :class="[
                selectedSet.has(row.hash) ? 'bg-[var(--row-selected)]' : 'hover:bg-[var(--row-hover)]',
                filtering && !matchSet.has(index) && row.kind !== 'wip' ? 'opacity-35' : '',
              ]"
              :style="{ top: index * ROW_H + 'px', height: ROW_H + 'px' }"
              @click="onRowClick(row, $event)"
              @dblclick="onRowDblClick(row)"
              @contextmenu="onRowCtx(row)"
            >
              <!-- branch / tag -->
              <div class="flex h-full shrink-0 items-center overflow-hidden pl-1.5" :style="{ width: cols.refs + 'px' }">
                <RefBadges
                  v-if="repo.refsByTarget.get(row.hash)?.length"
                  :refs="repo.refsByTarget.get(row.hash)!"
                  :color="row.color"
                  :head-branch="repo.currentBranch"
                  @dblclick="(r) => actions.checkoutRef(r)"
                  @ctx="(r) => onBadgeCtx(r)"
                />
              </div>
              <!-- graph -->
              <div class="h-full shrink-0 overflow-hidden" :style="{ width: graphW + 'px' }">
                <GraphCell
                  :row="row"
                  :width="graphW"
                  :height="ROW_H"
                  :lane-width="LANE_W"
                  :has-refs="!!repo.refsByTarget.get(row.hash)?.length"
                  :selected="selectedSet.has(row.hash)"
                  :is-head="row.hash === repo.head?.oid"
                />
              </div>
              <!-- message -->
              <div class="flex min-w-0 flex-1 items-center gap-2 px-2">
                <template v-if="row.kind === 'wip'">
                  <span class="mono text-muted-foreground italic">{{ t('graph.wip') }}</span>
                  <span v-if="repo.status?.operation" class="rounded bg-warning/20 px-1.5 text-[11px] font-medium text-warning">
                    {{ repo.status.operation.kind.replace('_', '-') }} em andamento
                  </span>
                  <span class="flex items-center gap-2 text-[11.5px]">
                    <span v-if="wipSummary.mod" class="flex items-center gap-0.5 text-warning"><Pencil class="size-3" />{{ wipSummary.mod }}</span>
                    <span v-if="wipSummary.add" class="flex items-center gap-0.5 text-added"><Plus class="size-3" />{{ wipSummary.add }}</span>
                    <span v-if="wipSummary.del" class="flex items-center gap-0.5 text-removed"><Minus class="size-3" />{{ wipSummary.del }}</span>
                  </span>
                </template>
                <template v-else-if="row.kind === 'stash'">
                  <span class="rounded bg-muted px-1 text-[10.5px] text-muted-foreground">{{ row.stashRef }}</span>
                  <span class="truncate text-muted-foreground italic">{{ row.subject }}</span>
                </template>
                <span v-else class="truncate" :class="row.parents.length > 1 ? 'text-muted-foreground' : ''" :title="row.subject">{{ row.subject }}</span>
              </div>
              <!-- author -->
              <div v-if="cols.showAuthor" class="flex shrink-0 items-center gap-1.5 overflow-hidden px-2 text-muted-foreground" :style="{ width: cols.author + 'px' }">
                <template v-if="row.kind === 'commit'">
                  <span class="grid size-4 shrink-0 place-items-center rounded-full text-[7.5px] font-bold text-black/80" :style="avatarStyle(row.authorName)">{{ initials(row.authorName) }}</span>
                  <span class="truncate" :title="`${row.authorName} <${row.authorEmail}>`">{{ row.authorName }}</span>
                </template>
              </div>
              <div v-if="cols.showSha" class="mono shrink-0 px-2 text-[11.5px] text-faint" :style="{ width: cols.sha + 'px' }">
                {{ row.kind === 'commit' ? shortSha(row.hash) : '' }}
              </div>
              <div v-if="cols.showDate" class="shrink-0 truncate px-2 text-[11.5px] text-muted-foreground" :style="{ width: cols.date + 'px' }" :title="row.kind !== 'wip' ? absoluteTime(row.authorTime) : ''">
                {{ row.kind !== 'wip' ? relativeTime(row.authorTime) : '' }}
              </div>
              <!-- lane colour hint on the left edge for the selected row -->
              <span v-if="selectedSet.has(row.hash)" class="absolute top-0 left-0 h-full w-[3px]" :style="{ background: laneColor(row.color) }" />
            </div>
          </div>
          <div v-if="repo.loading.more" class="sticky bottom-0 flex items-center justify-center gap-2 bg-background/80 py-1 text-[11.5px] text-muted-foreground">
            <LoaderCircle class="size-3.5 animate-spin" /> Carregando mais commits…
          </div>
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent class="w-72">
        <template v-if="menu">
        <RefMenuItems v-if="menu.type === 'ref'" :target="menu.ref" :repo="repo" :actions="actions" />
        <CommitMenuItems v-else-if="menu.type === 'commit'" :row="menu.row" :repo="repo" :actions="actions" @compare="compare" />
        <template v-else-if="menu.type === 'wip'">
          <ContextMenuItem :disabled="!repo.hasChanges" @select="actions.stage('all')">Preparar todas as alterações</ContextMenuItem>
          <ContextMenuItem :disabled="!repo.hasChanges" @select="actions.stashSave()">Stash das alterações…</ContextMenuItem>
          <ContextMenuItem :disabled="!repo.status?.unstaged.length" variant="destructive" @select="actions.discard(repo.status?.unstaged ?? [])">Descartar alterações não preparadas…</ContextMenuItem>
        </template>
        <template v-else-if="menu.type === 'stash' && stashOf(menu.row)">
          <ContextMenuItem @select="actions.stashApply(stashOf((menu as any).row)!, true)">Pop (aplicar e remover)</ContextMenuItem>
          <ContextMenuItem @select="actions.stashApply(stashOf((menu as any).row)!, false)">Aplicar</ContextMenuItem>
          <ContextMenuItem @select="actions.stashBranch(stashOf((menu as any).row)!)">Criar branch a partir do stash…</ContextMenuItem>
          <ContextMenuItem variant="destructive" @select="actions.stashDrop(stashOf((menu as any).row)!)">Descartar stash…</ContextMenuItem>
        </template>
              </template>
        <ContextMenuItem v-else disabled>Nenhuma ação aqui</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  </div>
</template>
