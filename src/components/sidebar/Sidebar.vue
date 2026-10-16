<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import {
  Archive, ArchiveRestore, Cloud, Copy, Download, ExternalLink, FolderGit2, FolderOpen, GitBranchPlus, Globe, Laptop,
  Lock, LockOpen, Package, Pencil, Plus, PlugZap, Scissors, Search, Tag, Terminal, Trash2, X,
} from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from '@/components/ui/context-menu'
import RefMenuItems from '@/components/menus/RefMenuItems.vue'
import RefTree from './RefTree.vue'
import SidebarSection from './SidebarSection.vue'
import { copyText, useActions } from '@/features/actions'
import { api, toAppError } from '@/services/api'
import { relativeTime, shortSha } from '@/lib/format'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { GitRef, StashEntry, Worktree } from '@/types/git'
import { t } from '@/i18n'

const props = defineProps<{ repo: RepoStore }>()
const repo = props.repo
const ui = useUiStore()
const actions = useActions(repo)

const open = reactive({ local: true, remote: true, worktrees: true, stashes: true, tags: false, submodules: false })
const q = ref('')
const match = (s: string) => !q.value || s.toLowerCase().includes(q.value.toLowerCase())

const locals = computed(() => repo.localBranches.filter((r) => match(r.short)))
const tags = computed(() => [...repo.tags].sort((a, b) => b.date - a.date).filter((r) => match(r.short)))
const remotesGrouped = computed(() => {
  const groups = new Map<string, GitRef[]>()
  for (const r of repo.remotes) groups.set(r.name, [])
  for (const r of repo.remoteBranches) {
    if (!match(r.short)) continue
    const key = r.remote ?? '?'
    const list = groups.get(key) ?? []
    list.push(r)
    groups.set(key, list)
  }
  return [...groups.entries()]
})
const worktrees = computed(() => repo.worktrees.filter((w) => match(w.path) || match(w.branch ?? '')))
const stashes = computed(() => repo.stashes.filter((s) => match(s.message) || match(s.reference)))
const remoteCollapsed = ref<Set<string>>(new Set())

const selectedTarget = computed(() => (repo.selection.kind === 'commits' ? repo.selection.hashes[0] : null))

async function selectRef(r: GitRef) {
  if (!repo.rowIndex.has(r.target)) {
    const ok = await repo.ensureLoaded(r.target, 30000)
    if (!ok) {
      ui.toast('info', `O commit de '${r.short}' não está no filtro atual do grafo.`, 'Mude o filtro para "Todas as branches".')
      return
    }
  }
  repo.selectCommit(r.target)
}
function selectStash(s: StashEntry) {
  if (repo.rowIndex.has(s.hash)) repo.selectCommit(s.hash)
  else repo.selection = { kind: 'stash', hash: s.hash }
}

type Menu =
  | { type: 'ref'; ref: GitRef }
  | { type: 'worktree'; wt: Worktree }
  | { type: 'stash'; s: StashEntry }
  | { type: 'remote'; name: string }
  | { type: 'section'; name: 'local' | 'remote' | 'worktrees' | 'stashes' | 'tags' }
const menu = ref<Menu | null>(null)

function reveal(path: string) {
  api.openInExplorer(path).catch((e) => ui.error(toAppError(e)))
}
function terminal(path: string) {
  api.openTerminal(path).catch((e) => ui.error(toAppError(e)))
}
function updateSubmodules() {
  void repo.run('Atualizar submódulos', (op) => api.submoduleUpdate(repo.path, op), { network: true })
}
function pushAllTags() {
  const remote = actions.defaultRemote()
  if (remote) void repo.run('Enviar tags', (op) => api.pushTag(repo.path, remote, null, op), { network: true })
}
const remoteInfo = (name: string) => repo.remotes.find((r) => r.name === name)
const base = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-panel">
    <div class="shrink-0 border-b p-2">
      <div class="relative">
        <Search class="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <input
          v-model="q"
          data-sidebar-filter
          class="h-7 w-full rounded-md border border-input bg-background pr-7 pl-7 text-[12.5px] outline-none focus:border-ring"
          :placeholder="t('sidebar.filter')"
          @keydown.esc="q = ''"
        />
        <button v-if="q" class="absolute top-1/2 right-1.5 -translate-y-1/2 rounded p-0.5 hover:bg-muted" @click="q = ''"><X class="size-3.5" /></button>
      </div>
    </div>

    <ContextMenu>
      <ContextMenuTrigger as-child>
        <div class="min-h-0 flex-1 overflow-auto" @contextmenu.self="menu = null">
          <!-- LOCAL -->
          <SidebarSection :title="t('sidebar.local')" :icon="Laptop" :count="repo.localBranches.length" :open="open.local" @toggle="open.local = !open.local" @ctx="menu = { type: 'section', name: 'local' }">
            <template #actions>
              <Button size="icon-xs" variant="ghost" class="opacity-0 group-hover:opacity-100" title="Nova branch" @click="actions.createBranch()"><Plus /></Button>
            </template>
            <RefTree :refs="locals" :selected-target="selectedTarget" @select="selectRef" @open="(r) => actions.checkoutRef(r)" @ctx="(r) => (menu = { type: 'ref', ref: r })" />
            <p v-if="!locals.length" class="px-4 py-1 text-[12px] text-muted-foreground">{{ q ? 'Nada encontrado.' : 'Sem branches locais.' }}</p>
          </SidebarSection>

          <!-- REMOTE -->
          <SidebarSection :title="t('sidebar.remote')" :icon="Cloud" :count="repo.remoteBranches.length" :open="open.remote" @toggle="open.remote = !open.remote" @ctx="menu = { type: 'section', name: 'remote' }">
            <template #actions>
              <Button size="icon-xs" variant="ghost" class="opacity-0 group-hover:opacity-100" title="Adicionar remote" @click="actions.addRemote()"><Plus /></Button>
            </template>
            <template v-for="[name, refs] in remotesGrouped" :key="name">
              <div
                class="flex h-6 cursor-default items-center gap-1.5 pr-2 pl-3 text-[12.5px] hover:bg-[var(--row-hover)]"
                :title="`fetch: ${remoteInfo(name)?.fetchUrl ?? '?'}\npush: ${remoteInfo(name)?.pushUrl ?? remoteInfo(name)?.fetchUrl ?? '?'}`"
                @click="remoteCollapsed.has(name) ? remoteCollapsed.delete(name) : remoteCollapsed.add(name)"
                @contextmenu="menu = { type: 'remote', name }"
              >
                <Globe class="size-3.5 shrink-0 text-muted-foreground" />
                <span class="truncate font-medium">{{ name }}</span>
                <span class="ml-auto truncate text-[10.5px] text-faint">{{ refs.length }}</span>
              </div>
              <RefTree
                v-if="!remoteCollapsed.has(name)"
                :refs="refs"
                :strip="name + '/'"
                :depth="1"
                :selected-target="selectedTarget"
                @select="selectRef"
                @open="(r) => actions.checkoutRef(r)"
                @ctx="(r) => (menu = { type: 'ref', ref: r })"
              />
            </template>
            <p v-if="!repo.remotes.length" class="px-4 py-1 text-[12px] text-muted-foreground">
              Sem remotes. <button class="text-primary hover:underline" @click="actions.addRemote()">Adicionar</button>
            </p>
          </SidebarSection>

          <!-- WORKTREES -->
          <SidebarSection :title="t('sidebar.worktrees')" :icon="FolderGit2" :count="repo.worktrees.length" :open="open.worktrees" @toggle="open.worktrees = !open.worktrees" @ctx="menu = { type: 'section', name: 'worktrees' }">
            <template #actions>
              <Button size="icon-xs" variant="ghost" class="opacity-0 group-hover:opacity-100" title="Criar worktree" @click="actions.createWorktree()"><Plus /></Button>
            </template>
            <div
              v-for="w in worktrees"
              :key="w.path"
              class="flex cursor-default items-start gap-2 py-1 pr-2 pl-4 text-[12.5px]"
              :class="w.isCurrent ? 'bg-primary/10' : 'hover:bg-[var(--row-hover)]'"
              :title="w.path"
              @dblclick="!w.isCurrent && actions.openWorktree(w)"
              @contextmenu="menu = { type: 'worktree', wt: w }"
            >
              <span class="mt-1.5 size-2 shrink-0 rounded-full" :class="w.prunable ? 'bg-destructive' : w.dirty ? 'bg-warning' : w.dirty === false ? 'bg-success' : 'bg-muted-foreground'" :title="w.prunable ? 'Pasta inexistente' : w.dirty ? `${w.changedFiles} alteração(ões)` : 'Limpa'" />
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-1">
                  <span class="truncate" :class="w.isCurrent ? 'font-semibold' : ''">{{ w.branch ?? (w.bare ? '(bare)' : `detached ${shortSha(w.head)}`) }}</span>
                  <Lock v-if="w.locked" class="size-3 shrink-0 text-warning" />
                  <span v-if="w.isMain" class="text-[10px] text-faint">principal</span>
                </div>
                <div class="truncate text-[11px] text-muted-foreground">{{ base(w.path) }}{{ w.isCurrent ? ' · esta aba' : '' }}</div>
              </div>
            </div>
            <button class="mx-3 mt-1 mb-1 flex w-[calc(100%-1.5rem)] items-center justify-center gap-1.5 rounded-md border border-dashed py-1 text-[12px] text-muted-foreground hover:border-primary hover:text-primary" @click="actions.createWorktree()">
              <Plus class="size-3.5" /> Criar worktree
            </button>
          </SidebarSection>

          <!-- STASHES -->
          <SidebarSection :title="t('sidebar.stashes')" :icon="Archive" :count="repo.stashes.length" :open="open.stashes" @toggle="open.stashes = !open.stashes" @ctx="menu = { type: 'section', name: 'stashes' }">
            <template #actions>
              <Button size="icon-xs" variant="ghost" class="opacity-0 group-hover:opacity-100" title="Novo stash" @click="actions.stashSave()"><Plus /></Button>
            </template>
            <div
              v-for="s in stashes"
              :key="s.hash"
              class="flex h-6 cursor-default items-center gap-1.5 pr-2 pl-4 text-[12.5px]"
              :class="repo.selection.kind === 'stash' && repo.selection.hash === s.hash ? 'bg-[var(--row-selected)]' : 'hover:bg-[var(--row-hover)]'"
              :title="`${s.reference} · ${s.branch ?? ''} · ${relativeTime(s.time)}`"
              @click="selectStash(s)"
              @dblclick="actions.stashApply(s, false)"
              @contextmenu="menu = { type: 'stash', s }"
            >
              <Archive class="size-3.5 shrink-0 text-muted-foreground" />
              <span class="truncate">{{ s.message }}</span>
              <span class="ml-auto shrink-0 text-[10.5px] text-faint">{{ relativeTime(s.time) }}</span>
            </div>
            <p v-if="!stashes.length" class="px-4 py-1 text-[12px] text-muted-foreground">Nenhum stash.</p>
          </SidebarSection>

          <!-- TAGS -->
          <SidebarSection :title="t('sidebar.tags')" :icon="Tag" :count="repo.tags.length" :open="open.tags" @toggle="open.tags = !open.tags" @ctx="menu = { type: 'section', name: 'tags' }">
            <div
              v-for="r in tags"
              :key="r.name"
              class="flex h-6 cursor-default items-center gap-1.5 pr-2 pl-4 text-[12.5px]"
              :class="selectedTarget === r.target ? 'bg-[var(--row-selected)]' : 'hover:bg-[var(--row-hover)]'"
              :title="r.message ?? ''"
              @click="selectRef(r)"
              @contextmenu="menu = { type: 'ref', ref: r }"
            >
              <Tag class="size-3.5 shrink-0 text-muted-foreground" />
              <span class="truncate">{{ r.short }}</span>
              <span v-if="r.annotated" class="ml-auto text-[10px] text-faint">anotada</span>
            </div>
            <p v-if="!tags.length" class="px-4 py-1 text-[12px] text-muted-foreground">Nenhuma tag.</p>
          </SidebarSection>

          <!-- SUBMODULES -->
          <SidebarSection v-if="repo.submodules.length" :title="t('sidebar.submodules')" :icon="Package" :count="repo.submodules.length" :open="open.submodules" @toggle="open.submodules = !open.submodules">
            <template #actions>
              <Button size="xs" variant="ghost" class="h-5 text-[11px]" title="git submodule update --init --recursive" @click="updateSubmodules">Atualizar</Button>
            </template>
            <div v-for="s in repo.submodules" :key="s.path" class="flex h-6 items-center gap-1.5 pr-2 pl-4 text-[12.5px]" :title="`${s.hash} ${s.describe ?? ''}`">
              <Package class="size-3.5 shrink-0 text-muted-foreground" />
              <span class="truncate">{{ s.path }}</span>
              <span class="ml-auto text-[10.5px]" :class="s.state === 'ok' ? 'text-faint' : 'text-warning'">{{ s.state === 'ok' ? shortSha(s.hash) : s.state === 'uninitialized' ? 'não iniciado' : s.state === 'modified' ? 'alterado' : 'conflito' }}</span>
            </div>
          </SidebarSection>
        </div>
      </ContextMenuTrigger>

      <ContextMenuContent class="w-64">
        <template v-if="menu">
        <RefMenuItems v-if="menu.type === 'ref'" :target="menu.ref" :repo="repo" :actions="actions" />
        <template v-else-if="menu.type === 'worktree'">
          <ContextMenuItem :disabled="menu.wt.isCurrent" @select="actions.openWorktree((menu as any).wt)"><FolderOpen /> Abrir em nova aba</ContextMenuItem>
          <ContextMenuItem @select="reveal((menu as any).wt.path)"><ExternalLink /> Mostrar no Explorer</ContextMenuItem>
          <ContextMenuItem @select="terminal((menu as any).wt.path)"><Terminal /> Abrir terminal aqui</ContextMenuItem>
          <ContextMenuItem @select="copyText((menu as any).wt.path, 'Caminho copiado.')"><Copy /> Copiar caminho</ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem :disabled="menu.wt.isMain" @select="actions.lockWorktree((menu as any).wt)">
            <component :is="menu.wt.locked ? LockOpen : Lock" /> {{ menu.wt.locked ? 'Desbloquear' : 'Bloquear…' }}
          </ContextMenuItem>
          <ContextMenuItem :disabled="menu.wt.isMain || menu.wt.isCurrent" variant="destructive" @select="actions.removeWorktree((menu as any).wt)"><Trash2 /> Remover worktree…</ContextMenuItem>
        </template>
        <template v-else-if="menu.type === 'stash'">
          <ContextMenuItem @select="actions.stashApply((menu as any).s, true)"><ArchiveRestore /> Pop (aplicar e remover)</ContextMenuItem>
          <ContextMenuItem @select="actions.stashApply((menu as any).s, false)">Aplicar</ContextMenuItem>
          <ContextMenuItem @select="actions.stashBranch((menu as any).s)"><GitBranchPlus /> Criar branch a partir do stash…</ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem variant="destructive" @select="actions.stashDrop((menu as any).s)"><Trash2 /> Descartar stash…</ContextMenuItem>
        </template>
        <template v-else-if="menu.type === 'remote'">
          <ContextMenuItem @select="actions.fetchAll((menu as any).name)"><Download /> Fetch {{ menu.name }}</ContextMenuItem>
          <ContextMenuItem @select="actions.pruneRemote((menu as any).name)"><Scissors /> Prune (remover refs obsoletas)</ContextMenuItem>
          <ContextMenuItem @select="actions.testRemote((menu as any).name)"><PlugZap /> Testar conexão/credenciais</ContextMenuItem>
          <ContextMenuItem v-if="remoteInfo(menu.name)?.webUrl" @select="actions.openRemoteWeb((menu as any).name)"><Globe /> Abrir no navegador</ContextMenuItem>
          <ContextMenuItem @select="copyText(remoteInfo((menu as any).name)?.fetchUrl ?? '', 'URL copiada.')"><Copy /> Copiar URL</ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem @select="actions.editRemote((menu as any).name)"><Pencil /> Editar…</ContextMenuItem>
          <ContextMenuItem variant="destructive" @select="actions.removeRemote((menu as any).name)"><Trash2 /> Remover remote…</ContextMenuItem>
        </template>
        <template v-else-if="menu.type === 'section'">
          <ContextMenuItem v-if="menu.name === 'local'" @select="actions.createBranch()"><Plus /> Nova branch…</ContextMenuItem>
          <ContextMenuItem v-if="menu.name === 'remote'" @select="actions.addRemote()"><Plus /> Adicionar remote…</ContextMenuItem>
          <ContextMenuItem v-if="menu.name === 'remote'" @select="actions.fetchAll(null, true)"><Download /> Fetch de todos com prune</ContextMenuItem>
          <ContextMenuItem v-if="menu.name === 'worktrees'" @select="actions.createWorktree()"><Plus /> Criar worktree…</ContextMenuItem>
          <ContextMenuItem v-if="menu.name === 'worktrees'" @select="actions.pruneWorktrees()"><Scissors /> Limpar worktrees obsoletas (prune)</ContextMenuItem>
          <ContextMenuItem v-if="menu.name === 'stashes'" @select="actions.stashSave()"><Plus /> Novo stash…</ContextMenuItem>
          <ContextMenuItem v-if="menu.name === 'stashes'" :disabled="!repo.stashes.length" variant="destructive" @select="actions.stashClear()"><Trash2 /> Limpar todos…</ContextMenuItem>
          <ContextMenuItem v-if="menu.name === 'tags'" :disabled="!repo.remotes.length" @select="pushAllTags"><Tag /> Enviar todas as tags</ContextMenuItem>
        </template>
              </template>
        <ContextMenuItem v-else disabled>Nenhuma ação aqui</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  </div>
</template>
