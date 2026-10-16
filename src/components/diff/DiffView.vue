<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  ArrowLeft, Columns2, ExternalLink, FileWarning, History, LoaderCircle, Maximize2, Minimize2, Minus, Plus, Rows2,
  ScanText, Trash2, WrapText, Check,
} from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { useActions } from '@/features/actions'
import { api, toAppError } from '@/services/api'
import { highlightLine, languageFor } from '@/lib/highlight'
import type { DiffTarget, RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { AppError, DiffHunk, DiffLine, FileChange, FileDiff, HunkSelection, LineAction } from '@/types/git'
import FileHistoryDialog from '@/components/dialogs/FileHistoryDialog.vue'

const props = defineProps<{ repo: RepoStore; target: Extract<DiffTarget, { kind: 'working' | 'commit' }> }>()
const repo = props.repo
const ui = useUiStore()
const actions = useActions(repo)

const diff = ref<FileDiff | null>(null)
const conflictText = ref<string | null>(null)
const loading = ref(false)
const error = ref<AppError | null>(null)
const selected = ref<Map<number, Set<number>>>(new Map())
let lastClicked: { hunk: number; line: number } | null = null

const isWorking = computed(() => props.target.kind === 'working')
const source = computed(() => (props.target.kind === 'working' ? props.target.source : null))
const fileEntry = computed<FileChange | null>(() => {
  if (props.target.kind !== 'working') return null
  const st = repo.status
  if (!st) return null
  const p = props.target.path
  if (source.value === 'staged') return st.staged.find((f) => f.path === p) ?? null
  return st.conflicted.find((f) => f.path === p) ?? st.unstaged.find((f) => f.path === p) ?? null
})
const isConflict = computed(() => fileEntry.value?.status === 'conflicted')
const lang = computed(() => languageFor(props.target.path))
const ctx = computed(() => ui.prefs.diffContext)

let token = 0
async function load() {
  const my = ++token
  loading.value = !diff.value
  error.value = null
  try {
    if (isConflict.value) {
      conflictText.value = await api.readWorktreeFile(repo.path, props.target.path)
      diff.value = null
    } else {
      conflictText.value = null
      const t = props.target
      const d = t.kind === 'working'
        ? await api.getWorkingDiff(repo.path, t.path, t.source, ctx.value)
        : await api.getCommitDiff(repo.path, t.base, t.sha, t.path, ctx.value)
      if (my !== token) return
      diff.value = d
    }
    selected.value = new Map()
  } catch (e) {
    if (my !== token) return
    error.value = toAppError(e)
    diff.value = null
  } finally {
    if (my === token) loading.value = false
  }
}

watch(() => [props.target, ctx.value], () => {
  diff.value = null
  void load()
}, { immediate: true, deep: true })
// Working tree diffs follow external edits / staging.
watch(() => repo.status, () => {
  if (isWorking.value) void load()
})

// ------------------------------------------------------------------ highlighting
const highlighted = computed(() => {
  const d = diff.value
  if (!d) return [] as string[][]
  const total = d.hunks.reduce((n, h) => n + h.lines.length, 0)
  const l = total > 6000 ? null : lang.value
  return d.hunks.map((h) => h.lines.map((ln) => (ln.kind === 'no_newline' ? ln.content : highlightLine(ln.content, l))))
})

// ------------------------------------------------------------------ split view rows
interface SplitCell { line: DiffLine | null; idx: number }
const splitRows = computed(() =>
  (diff.value?.hunks ?? []).map((h) => {
    const rows: { left: SplitCell; right: SplitCell }[] = []
    let i = 0
    while (i < h.lines.length) {
      const ln = h.lines[i]
      if (ln.kind === 'context' || ln.kind === 'no_newline') {
        rows.push({ left: { line: ln, idx: i }, right: { line: ln, idx: i } })
        i++
        continue
      }
      const dels: number[] = []
      const adds: number[] = []
      while (i < h.lines.length && h.lines[i].kind === 'del') dels.push(i++)
      while (i < h.lines.length && h.lines[i].kind === 'add') adds.push(i++)
      const n = Math.max(dels.length, adds.length)
      for (let k = 0; k < n; k++) {
        rows.push({
          left: { line: dels[k] != null ? h.lines[dels[k]] : null, idx: dels[k] ?? -1 },
          right: { line: adds[k] != null ? h.lines[adds[k]] : null, idx: adds[k] ?? -1 },
        })
      }
    }
    return rows
  }),
)

// ------------------------------------------------------------------ selection & staging
const canEditLines = computed(() => isWorking.value && source.value !== 'untracked' && !diff.value?.binary && !isConflict.value)
const selectedCount = computed(() => [...selected.value.values()].reduce((n, s) => n + s.size, 0))

function toggleLine(hunk: number, line: number, e: MouseEvent) {
  if (!canEditLines.value) return
  const h = diff.value!.hunks[hunk]
  if (h.lines[line].kind !== 'add' && h.lines[line].kind !== 'del') return
  const map = new Map(selected.value)
  const set = new Set(map.get(hunk) ?? [])
  if (e.shiftKey && lastClicked && lastClicked.hunk === hunk) {
    const [lo, hi] = lastClicked.line < line ? [lastClicked.line, line] : [line, lastClicked.line]
    for (let i = lo; i <= hi; i++) if (h.lines[i].kind === 'add' || h.lines[i].kind === 'del') set.add(i)
  } else if (set.has(line)) set.delete(line)
  else set.add(line)
  if (set.size) map.set(hunk, set)
  else map.delete(hunk)
  selected.value = map
  lastClicked = { hunk, line }
}
const isSel = (hunk: number, line: number) => selected.value.get(hunk)?.has(line) ?? false

async function applyLines(action: LineAction, hunks: { hunk: number; lines: number[] | null }[]) {
  const d = diff.value
  if (!d) return
  if (action === 'discard') {
    const n = hunks.reduce((acc, h) => acc + (h.lines?.length ?? d.hunks[h.hunk].lines.filter((l) => l.kind !== 'context').length), 0)
    const c = await ui.confirm({
      title: 'Descartar alterações selecionadas',
      message: `${n} linha(s) alterada(s) em ${d.path} voltarão ao conteúdo do index. Esta ação não pode ser desfeita.`,
      confirmLabel: 'Descartar',
      destructive: true,
    })
    if (!c.ok) return
  }
  const selections: HunkSelection[] = hunks.map((h) => ({ hunk: h.hunk, header: d.hunks[h.hunk].header, lines: h.lines }))
  const label = action === 'stage' ? 'Preparar' : action === 'unstage' ? 'Remover do stage' : 'Descartar'
  await repo.run(label, () => api.applyLines(repo.path, d.path, action, selections, ctx.value), { quiet: action !== 'discard' })
  selected.value = new Map()
}
function applySelection(action: LineAction) {
  void applyLines(action, [...selected.value.entries()].map(([hunk, set]) => ({ hunk, lines: [...set].sort((a, b) => a - b) })))
}

function hunkStats(h: DiffHunk) {
  let a = 0
  let d = 0
  for (const l of h.lines) {
    if (l.kind === 'add') a++
    else if (l.kind === 'del') d++
  }
  return { a, d }
}

// ------------------------------------------------------------------ file level
async function stageFile() {
  if (fileEntry.value) await actions.stage([fileEntry.value])
}
async function unstageFile() {
  if (fileEntry.value) await actions.unstage([fileEntry.value])
}
async function discardFile() {
  if (fileEntry.value) await actions.discard([fileEntry.value])
}
function openExternal() {
  api.openRepoFile(repo.path, props.target.path).catch((e) => ui.error(toAppError(e)))
}
function history() {
  void ui.openDialog(FileHistoryDialog, { repo, path: props.target.path })
}
function blame() {
  repo.openDiff({ kind: 'blame', path: props.target.path, rev: props.target.kind === 'commit' ? props.target.sha : null })
}

const title = computed(() => {
  const t = props.target
  if (t.kind === 'working') return t.source === 'staged' ? 'Preparado (index → HEAD)' : t.source === 'untracked' ? 'Novo arquivo (não rastreado)' : 'Working directory (vs. index)'
  return t.label ?? `Commit ${t.sha.slice(0, 8)}`
})

const conflictLines = computed(() => {
  if (!conflictText.value) return []
  let zone: 'ours' | 'theirs' | 'base' | null = null
  return conflictText.value.split(/\r?\n/).map((text, i) => {
    let cls = ''
    if (text.startsWith('<<<<<<<')) {
      zone = 'ours'
      cls = 'marker'
    } else if (text.startsWith('|||||||')) {
      zone = 'base'
      cls = 'marker'
    } else if (text.startsWith('=======') && zone) {
      zone = 'theirs'
      cls = 'marker'
    } else if (text.startsWith('>>>>>>>') && zone) {
      zone = null
      cls = 'marker'
    } else cls = zone ?? ''
    return { n: i + 1, html: highlightLine(text, lang.value), cls }
  })
})

const lineBg = (l: DiffLine | null) =>
  !l ? 'bg-muted/30' : l.kind === 'add' ? 'bg-[var(--diff-add-bg)]' : l.kind === 'del' ? 'bg-[var(--diff-del-bg)]' : ''
const marker = (l: DiffLine) => (l.kind === 'add' ? '+' : l.kind === 'del' ? '−' : l.kind === 'no_newline' ? '' : ' ')
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-background">
    <!-- header -->
    <div class="flex h-10 shrink-0 items-center gap-2 border-b bg-panel px-2">
      <Button size="icon-sm" variant="ghost" title="Voltar ao grafo (Esc)" @click="repo.closeDiff()"><ArrowLeft /></Button>
      <div class="min-w-0">
        <div class="flex items-center gap-2">
          <span class="truncate font-mono text-[12.5px] font-medium">{{ target.path }}</span>
          <span v-if="diff?.isNew" class="rounded bg-added/15 px-1 text-[10.5px] text-added">novo</span>
          <span v-if="diff?.isDeleted" class="rounded bg-removed/15 px-1 text-[10.5px] text-removed">removido</span>
          <span v-if="diff?.isRename" class="rounded bg-info/15 px-1 text-[10.5px] text-info">renomeado de {{ diff.oldPath }}</span>
          <span v-if="diff?.oldMode && diff?.newMode && diff.oldMode !== diff.newMode" class="rounded bg-muted px-1 text-[10.5px]">modo {{ diff.oldMode }} → {{ diff.newMode }}</span>
          <span v-if="isConflict" class="rounded bg-destructive/15 px-1 text-[10.5px] text-destructive">conflito</span>
        </div>
        <div class="truncate text-[11px] text-muted-foreground">{{ title }}</div>
      </div>
      <span v-if="diff" class="mono ml-1 text-[11.5px]"><span class="text-added">+{{ diff.additions }}</span> <span class="text-removed">−{{ diff.deletions }}</span></span>
      <LoaderCircle v-if="loading" class="size-4 animate-spin text-muted-foreground" />

      <div class="ml-auto flex items-center gap-1">
        <template v-if="isWorking && !isConflict">
          <template v-if="selectedCount">
            <span class="mr-1 text-[11.5px] text-muted-foreground">{{ selectedCount }} linha(s)</span>
            <Button v-if="source === 'unstaged'" size="xs" @click="applySelection('stage')"><Plus /> Preparar linhas</Button>
            <Button v-if="source === 'unstaged'" size="xs" variant="destructive" @click="applySelection('discard')"><Trash2 /> Descartar linhas</Button>
            <Button v-if="source === 'staged'" size="xs" @click="applySelection('unstage')"><Minus /> Remover linhas</Button>
            <Button size="xs" variant="ghost" @click="selected = new Map()">Limpar</Button>
          </template>
          <template v-else-if="fileEntry">
            <Button v-if="source !== 'staged'" size="xs" variant="outline" class="border-success/50 text-success" @click="stageFile"><Plus /> Preparar arquivo</Button>
            <Button v-else size="xs" variant="outline" @click="unstageFile"><Minus /> Remover do stage</Button>
            <Button v-if="source !== 'staged'" size="xs" variant="ghost" class="hover:text-destructive" title="Descartar alterações do arquivo" @click="discardFile"><Trash2 /></Button>
          </template>
        </template>
        <div class="mx-1 h-5 w-px bg-border" />
        <Button size="icon-xs" variant="ghost" :class="ui.prefs.diffMode === 'unified' ? 'bg-muted' : ''" title="Unificado" @click="ui.prefs.diffMode = 'unified'"><Rows2 /></Button>
        <Button size="icon-xs" variant="ghost" :class="ui.prefs.diffMode === 'split' ? 'bg-muted' : ''" title="Lado a lado" @click="ui.prefs.diffMode = 'split'"><Columns2 /></Button>
        <Button size="icon-xs" variant="ghost" :class="ui.prefs.wordWrap ? 'bg-muted' : ''" title="Quebra de linha" @click="ui.prefs.wordWrap = !ui.prefs.wordWrap"><WrapText /></Button>
        <div class="flex items-center rounded-md border text-[11px]" title="Linhas de contexto">
          <button class="px-1.5 py-0.5 hover:bg-muted" @click="ui.prefs.diffContext = Math.max(0, ui.prefs.diffContext - 2)">−</button>
          <span class="tabular-nums">{{ ui.prefs.diffContext }}</span>
          <button class="px-1.5 py-0.5 hover:bg-muted" @click="ui.prefs.diffContext = Math.min(200, ui.prefs.diffContext + 2)">+</button>
        </div>
        <Button size="icon-xs" variant="ghost" title="Histórico do arquivo" @click="history"><History /></Button>
        <Button size="icon-xs" variant="ghost" title="Blame" @click="blame"><ScanText /></Button>
        <Button v-if="isWorking" size="icon-xs" variant="ghost" title="Abrir no editor/app padrão" @click="openExternal"><ExternalLink /></Button>
        <Button size="icon-xs" variant="ghost" :title="repo.diffMaximized ? 'Restaurar painéis' : 'Maximizar diff'" @click="repo.diffMaximized = !repo.diffMaximized">
          <component :is="repo.diffMaximized ? Minimize2 : Maximize2" />
        </Button>
      </div>
    </div>

    <!-- body -->
    <div class="selectable min-h-0 flex-1 overflow-auto font-mono text-[12px] leading-[19px]">
      <div v-if="error" class="m-4 rounded-lg border border-destructive/40 bg-destructive/10 p-3 font-sans text-[12.5px]">
        <div class="font-medium text-destructive">{{ error.message }}</div>
        <div v-if="error.recovery" class="mt-1 text-muted-foreground">{{ error.recovery }}</div>
      </div>

      <!-- conflict view -->
      <template v-else-if="isConflict && conflictText !== null">
        <div class="sticky top-0 z-10 flex flex-wrap items-center gap-2 border-b bg-destructive/10 px-3 py-2 font-sans text-[12.5px]">
          <FileWarning class="size-4 text-destructive" />
          <span>Conflito ({{ fileEntry?.conflict }}). Edite o arquivo no seu editor ou escolha uma versão inteira:</span>
          <Button size="xs" variant="outline" @click="actions.resolve([fileEntry!], 'ours')">Usar a nossa</Button>
          <Button size="xs" variant="outline" @click="actions.resolve([fileEntry!], 'theirs')">Usar a deles</Button>
          <Button size="xs" variant="outline" @click="openExternal"><ExternalLink /> Abrir no editor</Button>
          <Button size="xs" @click="actions.resolve([fileEntry!], 'mark_resolved')"><Check /> Marcar resolvido</Button>
        </div>
        <div v-for="l in conflictLines" :key="l.n" class="flex" :class="{
          'bg-[var(--diff-add-bg)]': l.cls === 'ours',
          'bg-info/15': l.cls === 'theirs',
          'bg-muted': l.cls === 'base',
          'bg-warning/25 font-semibold': l.cls === 'marker',
        }">
          <span class="w-12 shrink-0 pr-2 text-right text-faint select-none">{{ l.n }}</span>
          <span class="flex-1 px-2" :class="ui.prefs.wordWrap ? 'whitespace-pre-wrap break-all' : 'whitespace-pre'" v-html="l.html || ' '" />
        </div>
      </template>

      <template v-else-if="diff">
        <div v-if="diff.binary" class="m-6 font-sans text-[13px] text-muted-foreground">Arquivo binário — conteúdo não exibido.</div>
        <div v-else-if="diff.submodule" class="m-6 font-sans text-[13px] text-muted-foreground">Alteração de submódulo (ponteiro de commit).</div>
        <div v-else-if="!diff.hunks.length" class="m-6 font-sans text-[13px] text-muted-foreground">
          Sem diferenças de conteúdo{{ diff.oldMode !== diff.newMode ? ' (apenas permissões/modo)' : '' }}.
        </div>

        <template v-for="(h, hi) in diff.hunks" :key="hi">
          <!-- hunk header -->
          <div class="sticky top-0 z-[5] flex items-center gap-2 border-y bg-panel-2 px-2 py-1 font-sans text-[11.5px] text-muted-foreground">
            <span class="mono truncate">{{ h.header }}</span>
            <span class="mono"><span class="text-added">+{{ hunkStats(h).a }}</span> <span class="text-removed">−{{ hunkStats(h).d }}</span></span>
            <div v-if="canEditLines" class="ml-auto flex gap-1">
              <Button v-if="source === 'unstaged'" size="xs" variant="ghost" class="h-5 hover:text-destructive" @click="applyLines('discard', [{ hunk: hi, lines: null }])"><Trash2 /> Descartar hunk</Button>
              <Button v-if="source === 'unstaged'" size="xs" variant="outline" class="h-5 border-success/50 text-success" @click="applyLines('stage', [{ hunk: hi, lines: null }])"><Plus /> Preparar hunk</Button>
              <Button v-if="source === 'staged'" size="xs" variant="outline" class="h-5" @click="applyLines('unstage', [{ hunk: hi, lines: null }])"><Minus /> Remover hunk</Button>
            </div>
          </div>

          <!-- unified -->
          <template v-if="ui.prefs.diffMode === 'unified'">
            <div
              v-for="(l, li) in h.lines"
              :key="li"
              class="flex"
              :class="[lineBg(l), isSel(hi, li) ? 'outline-1 -outline-offset-1 outline-primary brightness-125' : '']"
            >
              <span
                class="w-11 shrink-0 pr-1.5 text-right text-faint select-none"
                :class="canEditLines && l.kind !== 'context' && l.kind !== 'no_newline' ? 'cursor-pointer hover:bg-primary/20' : ''"
                :title="canEditLines && l.kind !== 'context' ? 'Clique para selecionar a linha (Shift para intervalo)' : ''"
                @click="toggleLine(hi, li, $event)"
              >{{ l.oldNo ?? '' }}</span>
              <span
                class="w-11 shrink-0 pr-1.5 text-right text-faint select-none"
                :class="canEditLines && l.kind !== 'context' && l.kind !== 'no_newline' ? 'cursor-pointer hover:bg-primary/20' : ''"
                @click="toggleLine(hi, li, $event)"
              >{{ l.newNo ?? '' }}</span>
              <span class="w-4 shrink-0 text-center select-none" :class="l.kind === 'add' ? 'text-added' : l.kind === 'del' ? 'text-removed' : 'text-faint'">{{ marker(l) }}</span>
              <span
                v-if="l.kind === 'no_newline'"
                class="flex-1 px-1 font-sans text-[11px] text-muted-foreground italic"
              >Sem quebra de linha no final do arquivo</span>
              <span v-else class="flex-1 pr-4" :class="ui.prefs.wordWrap ? 'whitespace-pre-wrap break-all' : 'whitespace-pre'" v-html="highlighted[hi]?.[li] || ' '" />
            </div>
          </template>

          <!-- split -->
          <template v-else>
            <div v-for="(r, ri) in splitRows[hi]" :key="ri" class="grid grid-cols-2">
              <div class="flex min-w-0 border-r" :class="[lineBg(r.left.line), r.left.idx >= 0 && isSel(hi, r.left.idx) ? 'outline-1 -outline-offset-1 outline-primary' : '']">
                <span class="w-11 shrink-0 pr-1.5 text-right text-faint select-none" :class="canEditLines && r.left.line?.kind === 'del' ? 'cursor-pointer hover:bg-primary/20' : ''" @click="r.left.idx >= 0 && toggleLine(hi, r.left.idx, $event)">{{ r.left.line?.oldNo ?? '' }}</span>
                <span v-if="r.left.line" class="min-w-0 flex-1 overflow-hidden pr-2" :class="ui.prefs.wordWrap ? 'whitespace-pre-wrap break-all' : 'whitespace-pre'" v-html="highlighted[hi]?.[r.left.idx] || ' '" />
              </div>
              <div class="flex min-w-0" :class="[lineBg(r.right.line), r.right.idx >= 0 && isSel(hi, r.right.idx) && r.right.line?.kind === 'add' ? 'outline-1 -outline-offset-1 outline-primary' : '']">
                <span class="w-11 shrink-0 pr-1.5 text-right text-faint select-none" :class="canEditLines && r.right.line?.kind === 'add' ? 'cursor-pointer hover:bg-primary/20' : ''" @click="r.right.idx >= 0 && r.right.line?.kind === 'add' && toggleLine(hi, r.right.idx, $event)">{{ r.right.line?.newNo ?? '' }}</span>
                <span v-if="r.right.line" class="min-w-0 flex-1 overflow-hidden pr-2" :class="ui.prefs.wordWrap ? 'whitespace-pre-wrap break-all' : 'whitespace-pre'" v-html="highlighted[hi]?.[r.right.idx] || ' '" />
              </div>
            </div>
          </template>
        </template>
        <div v-if="diff.truncated" class="m-4 font-sans text-[12.5px] text-warning">Diff muito grande: exibindo apenas as primeiras 20.000 linhas.</div>
      </template>
    </div>
  </div>
</template>
