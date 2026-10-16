<script setup lang="ts">
// Ctrl+K: commands, branches, repositories and commit search in one list.
import { computed, nextTick, ref, watch } from 'vue'
import { CornerDownLeft, FolderGit2, GitBranch, GitCommitHorizontal, Search, Terminal } from '@lucide/vue'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { useActions } from '@/features/actions'
import { cloneRepo, initRepo, openRepoFolder } from '@/features/repos'
import { api, toAppError } from '@/services/api'
import { shortSha } from '@/lib/format'
import { useAppStore } from '@/stores/app'
import { useUiStore } from '@/stores/ui'
import AccountsDialog from '@/components/dialogs/AccountsDialog.vue'
import SettingsDialog from '@/components/dialogs/SettingsDialog.vue'

const app = useAppStore()
const ui = useUiStore()
const q = ref('')
const active = ref(0)
const input = ref<HTMLInputElement | null>(null)
const commitHits = ref<{ hash: string; subject: string }[]>([])

interface Item { id: string; group: string; label: string; hint?: string; icon: unknown; run: () => unknown }

const items = computed<Item[]>(() => {
  const out: Item[] = []
  const repo = app.activeRepo
  const add = (group: string, label: string, run: () => unknown, hint?: string, icon: unknown = Terminal) =>
    out.push({ id: `${group}:${label}`, group, label, run, hint, icon })
  if (repo) {
    const a = useActions(repo)
    add('Ações', 'Fetch de todos os remotes', () => a.fetchAll())
    add('Ações', 'Pull', () => a.pull())
    add('Ações', 'Pull com rebase', () => a.pull('rebase'))
    add('Ações', 'Push', () => a.push())
    add('Ações', 'Force push (with lease)…', () => a.push({ force: true }))
    add('Ações', 'Nova branch…', () => a.createBranch())
    add('Ações', 'Stash das alterações…', () => a.stashSave())
    add('Ações', 'Pop do último stash', () => a.stashPopLatest())
    add('Ações', 'Criar worktree…', () => a.createWorktree())
    add('Ações', 'Desfazer última operação', () => a.undo())
    add('Ações', 'Adicionar remote…', () => a.addRemote())
    add('Ações', 'Preparar todas as alterações', () => a.stage('all'))
    add('Ações', 'Atualizar (F5)', () => repo.loadAll())
    add('Ações', 'Abrir terminal no repositório', () => api.openTerminal(repo.path).catch((e) => ui.error(toAppError(e))))
    add('Ações', 'Abrir pasta no Explorer', () => api.openInExplorer(repo.path).catch((e) => ui.error(toAppError(e))))
    if (repo.status?.operation) {
      add('Ações', 'Continuar operação em andamento', () => a.sequencer('continue'))
      add('Ações', 'Abortar operação em andamento', () => a.sequencer('abort'))
    }
    for (const b of repo.localBranches) out.push({ id: `b:${b.name}`, group: 'Branches', label: b.short, hint: b.isHead ? 'atual' : 'checkout', icon: GitBranch, run: () => a.checkoutRef(b) })
    for (const b of repo.remoteBranches) out.push({ id: `r:${b.name}`, group: 'Branches remotas', label: b.short, hint: 'checkout', icon: GitBranch, run: () => a.checkoutRef(b) })
    for (const c of commitHits.value)
      out.push({ id: `c:${c.hash}`, group: 'Commits', label: c.subject, hint: shortSha(c.hash), icon: GitCommitHorizontal, run: async () => (await repo.ensureLoaded(c.hash)) && repo.selectCommit(c.hash) })
  }
  add('Geral', 'Abrir repositório…', () => openRepoFolder())
  add('Geral', 'Clonar repositório…', () => cloneRepo())
  add('Geral', 'Inicializar repositório…', () => initRepo())
  add('Geral', 'Contas conectadas…', () => ui.openDialog(AccountsDialog, {}))
  add('Geral', 'Configurações…', () => ui.openDialog(SettingsDialog, { repo: app.activeRepo }))
  add('Geral', `Tema ${ui.prefs.theme === 'dark' ? 'claro' : 'escuro'}`, () => (ui.prefs.theme = ui.prefs.theme === 'dark' ? 'light' : 'dark'))
  for (const r of app.recent) out.push({ id: `repo:${r.path}`, group: 'Repositórios', label: r.name, hint: r.path, icon: FolderGit2, run: () => app.openRepo(r.path) })
  return out
})

const filtered = computed(() => {
  const s = q.value.trim().toLowerCase()
  const list = s ? items.value.filter((i) => i.group === 'Commits' || (i.label + ' ' + (i.hint ?? '')).toLowerCase().includes(s)) : items.value.filter((i) => !i.group.startsWith('Branches remotas'))
  return list.slice(0, 200)
})

let searchTimer: ReturnType<typeof setTimeout> | undefined
watch(q, (s) => {
  active.value = 0
  clearTimeout(searchTimer)
  commitHits.value = []
  const repo = app.activeRepo
  if (!repo || s.trim().length < 3) return
  searchTimer = setTimeout(async () => {
    try {
      const hashes = await api.searchCommits(repo.path, s.trim(), repo.scope)
      commitHits.value = hashes.slice(0, 15).map((h) => ({ hash: h, subject: repo.rows[repo.rowIndex.get(h) ?? -1]?.subject ?? `Commit ${shortSha(h)}` }))
    } catch {
      /* ignore */
    }
  }, 300)
})

watch(() => ui.paletteOpen, async (o) => {
  if (o) {
    q.value = ''
    active.value = 0
    await nextTick()
    input.value?.focus()
  }
})

function run(i: Item | undefined) {
  if (!i) return
  ui.paletteOpen = false
  void i.run()
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    active.value = Math.min(filtered.value.length - 1, active.value + 1)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    active.value = Math.max(0, active.value - 1)
  } else if (e.key === 'Enter') {
    e.preventDefault()
    run(filtered.value[active.value])
  }
  nextTick(() => document.querySelector('[data-palette-active="true"]')?.scrollIntoView({ block: 'nearest' }))
}
</script>

<template>
  <Dialog :open="ui.paletteOpen" @update:open="(v: boolean) => (ui.paletteOpen = v)">
    <DialogContent class="top-[18%] translate-y-0 gap-0 overflow-hidden p-0 sm:max-w-xl" :show-close-button="false">
      <DialogTitle class="sr-only">Comandos</DialogTitle>
      <DialogDescription class="sr-only">Busque comandos, branches, repositórios e commits.</DialogDescription>
      <div class="flex items-center gap-2 border-b px-3">
        <Search class="size-4 text-muted-foreground" />
        <input ref="input" v-model="q" class="h-11 flex-1 bg-transparent text-[13.5px] outline-none" placeholder="Comando, branch, repositório ou commit…" @keydown="onKey" />
      </div>
      <div class="max-h-[50vh] overflow-auto py-1">
        <template v-for="(i, idx) in filtered" :key="i.id">
          <div v-if="idx === 0 || filtered[idx - 1].group !== i.group" class="px-3 pt-2 pb-1 text-[10.5px] font-semibold tracking-wider text-muted-foreground uppercase">{{ i.group }}</div>
          <button
            class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px]"
            :class="idx === active ? 'bg-accent' : ''"
            :data-palette-active="idx === active"
            @mousemove="active = idx"
            @click="run(i)"
          >
            <component :is="i.icon" class="size-4 shrink-0 text-muted-foreground" />
            <span class="truncate">{{ i.label }}</span>
            <span v-if="i.hint" class="ml-auto max-w-[45%] truncate text-[11px] text-muted-foreground">{{ i.hint }}</span>
            <CornerDownLeft v-if="idx === active" class="size-3.5 shrink-0 text-muted-foreground" />
          </button>
        </template>
        <p v-if="!filtered.length" class="px-3 py-6 text-center text-[12.5px] text-muted-foreground">Nada encontrado.</p>
      </div>
    </DialogContent>
  </Dialog>
</template>
