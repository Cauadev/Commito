<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  Archive, ArchiveRestore, ArrowDownToLine, ArrowUpFromLine, ChevronDown, CircleUser, Download, FolderGit2, GitBranch,
  GitBranchPlus, Search, Settings, SquareTerminal, Undo2, FolderOpen,
} from '@lucide/vue'
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import ToolButton from './ToolButton.vue'
import ToolButtonBase from './ToolButtonBase.vue'
import { useActions } from '@/features/actions'
import { api, toAppError } from '@/services/api'
import { useAppStore } from '@/stores/app'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { PullStrategy } from '@/types/git'
import { t } from '@/i18n'
import SettingsDialog from '@/components/dialogs/SettingsDialog.vue'
import AccountsDialog from '@/components/dialogs/AccountsDialog.vue'
import { openRepoFolder } from '@/features/repos'

const props = defineProps<{ repo: RepoStore }>()
const repo = props.repo
const app = useAppStore()
const ui = useUiStore()
const actions = useActions(repo)

const netBusy = computed(() => Object.values(ui.ops).some((o) => o.repo === repo.path))
const topUndo = computed(() => repo.undo[0] ?? null)
const head = computed(() => repo.status?.head)
const branchQuery = ref('')
const branchOpen = ref(false)
const branches = computed(() => {
  const q = branchQuery.value.toLowerCase()
  return repo.localBranches.filter((b) => b.short.toLowerCase().includes(q)).slice(0, 200)
})
const remoteMatches = computed(() => {
  const q = branchQuery.value.toLowerCase()
  if (!q) return []
  return repo.remoteBranches.filter((b) => b.short.toLowerCase().includes(q)).slice(0, 50)
})

const pullLabels: Record<PullStrategy, string> = {
  config: 'Usar configuração do Git',
  merge: 'Merge (padrão)',
  rebase: 'Rebase',
  ff_only: 'Somente fast-forward',
}

function terminal() {
  api.openTerminal(repo.path).catch((e) => ui.error(toAppError(e)))
}
</script>

<template>
  <div class="flex h-[54px] shrink-0 items-center gap-1 border-b bg-chrome px-2">
    <!-- repository -->
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <button class="flex h-11 max-w-56 flex-col items-start justify-center rounded-md px-2 text-left hover:bg-muted">
          <span class="text-[10.5px] leading-none text-muted-foreground">{{ t('toolbar.repository') }}</span>
          <span class="flex max-w-full items-center gap-1 text-[14px] leading-tight font-semibold">
            <span class="truncate">{{ repo.summary.name }}</span>
            <ChevronDown class="size-3.5 shrink-0 text-muted-foreground" />
          </span>
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" class="w-80">
        <DropdownMenuLabel class="truncate text-[11px] font-normal text-muted-foreground">{{ repo.path }}</DropdownMenuLabel>
        <DropdownMenuItem @select="api.openInExplorer(repo.path)"><FolderOpen /> Abrir pasta no Explorer</DropdownMenuItem>
        <DropdownMenuItem @select="terminal"><SquareTerminal /> Abrir terminal aqui</DropdownMenuItem>
        <DropdownMenuItem v-if="repo.remotes.some((r) => r.webUrl)" @select="actions.openRemoteWeb()">Abrir remote no navegador</DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuLabel class="text-[11px]">Recentes</DropdownMenuLabel>
        <DropdownMenuItem v-for="r in app.recent.slice(0, 10)" :key="r.path" @select="app.openRepo(r.path)">
          <FolderGit2 /> <span class="truncate">{{ r.name }}</span>
          <span class="ml-auto max-w-32 truncate text-[10.5px] text-muted-foreground">{{ r.path }}</span>
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem @select="openRepoFolder()"><FolderOpen /> Abrir outro repositório…</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>

    <span class="text-muted-foreground/50">/</span>

    <!-- branch -->
    <Popover v-model:open="branchOpen">
      <PopoverTrigger as-child>
        <button class="flex h-11 max-w-64 flex-col items-start justify-center rounded-md px-2 text-left hover:bg-muted">
          <span class="text-[10.5px] leading-none text-muted-foreground">{{ head?.detached ? 'HEAD destacado' : t('toolbar.branch') }}</span>
          <span class="flex max-w-full items-center gap-1 text-[14px] leading-tight font-semibold">
            <GitBranch class="size-3.5 shrink-0 text-primary" />
            <span class="truncate">{{ repo.currentBranch ?? (head?.oid ? head.oid.slice(0, 8) : '—') }}</span>
            <ChevronDown class="size-3.5 shrink-0 text-muted-foreground" />
          </span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" class="w-80 p-0">
        <div class="border-b p-2">
          <input v-model="branchQuery" class="h-7 w-full rounded-md border border-input bg-background px-2 text-[12.5px] outline-none focus:border-ring" placeholder="Trocar para branch…" autofocus />
        </div>
        <div class="max-h-80 overflow-auto py-1">
          <button
            v-for="b in branches"
            :key="b.name"
            class="flex w-full items-center gap-2 px-3 py-1 text-left text-[12.5px] hover:bg-muted"
            :class="b.isHead ? 'font-semibold text-primary' : ''"
            @click="branchOpen = false; actions.checkoutRef(b)"
          >
            <GitBranch class="size-3.5 shrink-0" /> <span class="truncate">{{ b.short }}</span>
            <span v-if="b.upstream" class="ml-auto truncate text-[10.5px] text-muted-foreground">{{ b.upstream }}</span>
          </button>
          <template v-if="remoteMatches.length">
            <div class="px-3 pt-2 pb-1 text-[10.5px] text-muted-foreground uppercase">Remotas</div>
            <button v-for="b in remoteMatches" :key="b.name" class="flex w-full items-center gap-2 px-3 py-1 text-left text-[12.5px] hover:bg-muted" @click="branchOpen = false; actions.checkoutRef(b)">
              <GitBranch class="size-3.5 shrink-0 text-muted-foreground" /> <span class="truncate">{{ b.short }}</span>
            </button>
          </template>
        </div>
        <div class="border-t p-1">
          <button class="flex w-full items-center gap-2 rounded px-2 py-1 text-[12.5px] hover:bg-muted" @click="branchOpen = false; actions.createBranch()">
            <GitBranchPlus class="size-3.5" /> Nova branch…
          </button>
        </div>
      </PopoverContent>
    </Popover>

    <div class="mx-auto flex h-full items-center gap-0.5 py-1">
      <ToolButton
        :label="t('toolbar.undo')"
        :icon="Undo2"
        :disabled="!topUndo || !!repo.busy"
        :tip="topUndo ? (topUndo.available ? `Desfazer: ${topUndo.label}` : `Indisponível: ${topUndo.reason}`) : 'Nada para desfazer (apenas operações locais reversíveis feitas no app)'"
        @click="actions.undo()"
      />
      <div class="mx-1 h-7 w-px bg-border" />
      <DropdownMenu>
        <DropdownMenuTrigger as-child>
          <ToolButtonBase :label="t('toolbar.fetch')" :icon="Download" :busy="netBusy" title="Fetch (buscar do remote)" />
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem @select="actions.fetchAll()">Fetch de todos os remotes</DropdownMenuItem>
          <DropdownMenuItem @select="actions.fetchAll(null, true)">Fetch com prune</DropdownMenuItem>
          <DropdownMenuSeparator v-if="repo.remotes.length > 1" />
          <DropdownMenuItem v-for="r in repo.remotes.length > 1 ? repo.remotes : []" :key="r.name" @select="actions.fetchAll(r.name)">Fetch {{ r.name }}</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <DropdownMenu>
        <DropdownMenuTrigger as-child>
          <ToolButtonBase :label="t('toolbar.pull')" :icon="ArrowDownToLine" :badge="head?.behind || null" :disabled="!!repo.busy" title="Pull" />
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuLabel class="text-[11px]">Pull da branch atual</DropdownMenuLabel>
          <DropdownMenuItem v-for="(label, s) in pullLabels" :key="s" @select="actions.pull(s as PullStrategy)">
            {{ label }}
            <span v-if="ui.prefs.pullStrategy === s" class="ml-auto text-[10.5px] text-primary">padrão</span>
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuLabel class="text-[11px]">Estratégia padrão do botão</DropdownMenuLabel>
          <DropdownMenuItem v-for="(label, s) in pullLabels" :key="'d' + s" @select="ui.prefs.pullStrategy = s as PullStrategy">
            {{ ui.prefs.pullStrategy === s ? '● ' : '○ ' }}{{ label }}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <ToolButton :label="t('toolbar.push')" :icon="ArrowUpFromLine" :badge="head?.ahead || null" :disabled="!!repo.busy" tip="Push da branch atual" @click="actions.push()" />
      <div class="mx-1 h-7 w-px bg-border" />
      <ToolButton :label="t('toolbar.branchNew')" :icon="GitBranchPlus" tip="Criar branch" @click="actions.createBranch()" />
      <ToolButton :label="t('toolbar.stash')" :icon="Archive" :disabled="!repo.hasChanges" tip="Guardar alterações no stash" @click="actions.stashSave()" />
      <ToolButton :label="t('toolbar.pop')" :icon="ArchiveRestore" :disabled="!repo.stashes.length" tip="Aplicar e remover o stash mais recente" @click="actions.stashPopLatest()" />
      <ToolButton :label="t('toolbar.worktree')" :icon="FolderGit2" tip="Criar worktree" @click="actions.createWorktree()" />
      <ToolButton :label="t('toolbar.terminal')" :icon="SquareTerminal" tip="Abrir terminal na pasta do repositório" @click="terminal" />
    </div>

    <ToolButton :label="t('toolbar.search')" :icon="Search" tip="Comandos e busca (Ctrl+K)" @click="ui.paletteOpen = true" />
    <ToolButton :label="t('toolbar.accounts')" :icon="CircleUser" tip="Contas conectadas (GitHub, GitLab, Bitbucket)" @click="ui.openDialog(AccountsDialog, {})" />
    <ToolButton :label="t('toolbar.settings')" :icon="Settings" tip="Configurações" @click="ui.openDialog(SettingsDialog, { repo })" />
  </div>
</template>
