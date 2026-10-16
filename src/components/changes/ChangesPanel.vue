<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  AlertTriangle, ChevronDown, ChevronRight, FolderTree, GitCommitHorizontal, List, LoaderCircle, Minus, Plus,
  SkipForward, Trash2, X, Check, UserCog,
} from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@/components/ui/context-menu'
import FileList from './FileList.vue'
import FileMenuItems from './FileMenuItems.vue'
import { useActions } from '@/features/actions'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { FileChange } from '@/types/git'
import { t } from '@/i18n'
import IdentityDialog from '@/components/dialogs/IdentityDialog.vue'

const props = defineProps<{ repo: RepoStore }>()
const repo = props.repo
const ui = useUiStore()
const actions = useActions(repo)

const st = computed(() => repo.status)
const unstaged = computed(() => st.value?.unstaged ?? [])
const staged = computed(() => st.value?.staged ?? [])
const conflicted = computed(() => st.value?.conflicted ?? [])
const op = computed(() => st.value?.operation ?? null)
const open = ref({ unstaged: true, staged: true, conflicted: true })

const activePath = computed(() => (repo.diffTarget?.kind === 'working' ? repo.diffTarget.path : null))
const activeSource = computed(() => (repo.diffTarget?.kind === 'working' ? repo.diffTarget.source : null))

function openFile(f: FileChange, section: 'unstaged' | 'staged' | 'conflicted') {
  const source = section === 'staged' ? 'staged' : f.status === 'untracked' ? 'untracked' : 'unstaged'
  repo.openDiff({ kind: 'working', path: f.path, source })
}

const menu = ref<{ files: FileChange[]; source: 'unstaged' | 'staged' | 'conflicted' } | null>(null)
function setMenu(files: FileChange[], source: 'unstaged' | 'staged' | 'conflicted') {
  menu.value = { files, source }
}

const opLabel = computed(() => {
  const o = op.value
  if (!o) return ''
  const names: Record<string, string> = { merge: 'Merge', rebase: 'Rebase', cherry_pick: 'Cherry-pick', revert: 'Revert', bisect: 'Bisect', apply_mailbox: 'git am' }
  const step = o.step && o.total ? ` (${o.step}/${o.total})` : ''
  return `${names[o.kind] ?? o.kind} em andamento${step}`
})
const canSkip = computed(() => op.value && op.value.kind !== 'merge')

// ---------------------------------------------------------------- composer
const summaryLen = computed(() => repo.composer.summary.length)
const identity = computed(() => repo.identity)
const canCommit = computed(() => !!repo.composer.summary.trim() && (staged.value.length > 0 || repo.composer.amend) && !conflicted.value.length && !repo.busy)
const committing = ref(false)
async function doCommit() {
  if (!canCommit.value) return
  committing.value = true
  try {
    await actions.commit()
  } finally {
    committing.value = false
  }
}
function onKey(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    e.preventDefault()
    void doCommit()
  }
}
async function editIdentity() {
  await ui.openDialog(IdentityDialog, { repo })
  await repo.refreshIdentity()
}
const showOptions = ref(false)
const totalFiles = computed(() => repo.changeCount)
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-panel">
    <!-- header -->
    <div class="flex h-9 shrink-0 items-center gap-2 border-b px-3">
      <span class="truncate text-[12.5px]">
        <b>{{ totalFiles }}</b> {{ totalFiles === 1 ? 'alteração' : 'alterações' }} em
        <span class="rounded bg-primary/15 px-1.5 py-0.5 font-medium text-primary">{{ repo.currentBranch ?? 'HEAD destacado' }}</span>
      </span>
      <LoaderCircle v-if="repo.loading.status" class="size-3.5 animate-spin text-muted-foreground" />
      <div class="ml-auto flex rounded-md border p-0.5">
        <button class="flex items-center gap-1 rounded px-1.5 py-0.5 text-[11.5px]" :class="ui.prefs.fileView === 'path' ? 'bg-muted text-foreground' : 'text-muted-foreground'" @click="ui.prefs.fileView = 'path'"><List class="size-3.5" />Path</button>
        <button class="flex items-center gap-1 rounded px-1.5 py-0.5 text-[11.5px]" :class="ui.prefs.fileView === 'tree' ? 'bg-muted text-foreground' : 'text-muted-foreground'" @click="ui.prefs.fileView = 'tree'"><FolderTree class="size-3.5" />Tree</button>
      </div>
    </div>

    <!-- in-progress operation -->
    <div v-if="op" class="shrink-0 border-b bg-warning/10 px-3 py-2">
      <div class="flex items-center gap-2 text-[12.5px] font-medium text-warning">
        <AlertTriangle class="size-4" /> {{ opLabel }}
      </div>
      <p class="mt-0.5 text-[11.5px] text-muted-foreground">
        {{ conflicted.length ? `Resolva ${conflicted.length} arquivo(s) em conflito e marque como resolvido.` : 'Sem conflitos pendentes: você pode continuar.' }}
      </p>
      <div class="mt-2 flex gap-1.5">
        <Button size="xs" :disabled="conflicted.length > 0 || !!repo.busy" @click="actions.sequencer('continue')"><Check /> Continuar</Button>
        <Button v-if="canSkip" size="xs" variant="outline" :disabled="!!repo.busy" @click="actions.sequencer('skip')"><SkipForward /> Pular</Button>
        <Button size="xs" variant="destructive" :disabled="!!repo.busy" @click="actions.sequencer('abort')"><X /> Abortar</Button>
      </div>
    </div>

    <ContextMenu>
      <ContextMenuTrigger as-child>
        <div class="min-h-0 flex-1 overflow-auto">
          <!-- conflicts -->
          <section v-if="conflicted.length">
            <div class="sticky top-0 z-10 flex h-8 items-center gap-1 border-b bg-panel px-2">
              <button class="flex items-center gap-1 text-[12.5px] font-semibold text-destructive" @click="open.conflicted = !open.conflicted">
                <component :is="open.conflicted ? ChevronDown : ChevronRight" class="size-4" /> {{ t('changes.conflicted') }} ({{ conflicted.length }})
              </button>
            </div>
            <FileList v-if="open.conflicted" actions-visible :files="conflicted" :mode="ui.prefs.fileView" :active-path="activeSource === 'unstaged' ? activePath : null" @open="(f) => openFile(f, 'conflicted')" @ctx="(files: FileChange[]) => setMenu(files, 'conflicted')">
              <template #actions="{ selection }">
                <Button size="xs" variant="ghost" title="Usar a nossa versão" @click="actions.resolve(selection, 'ours')">Nossa</Button>
                <Button size="xs" variant="ghost" title="Usar a versão deles" @click="actions.resolve(selection, 'theirs')">Deles</Button>
                <Button size="icon-xs" variant="ghost" title="Marcar como resolvido" @click="actions.resolve(selection, 'mark_resolved')"><Check /></Button>
              </template>
            </FileList>
          </section>

          <!-- unstaged -->
          <section>
            <div class="sticky top-0 z-10 flex h-8 items-center gap-1 border-b bg-panel px-2">
              <button class="flex items-center gap-1 text-[12.5px] font-semibold" @click="open.unstaged = !open.unstaged">
                <component :is="open.unstaged ? ChevronDown : ChevronRight" class="size-4" /> {{ t('changes.unstaged') }} ({{ unstaged.length }})
              </button>
              <div class="ml-auto flex gap-1">
                <Button v-if="unstaged.length" size="xs" variant="ghost" class="text-muted-foreground hover:text-destructive" title="Descartar todas as alterações não preparadas" @click="actions.discard(unstaged)"><Trash2 /></Button>
                <Button v-if="unstaged.length" size="xs" variant="outline" class="border-success/50 text-success hover:bg-success/10" @click="actions.stage('all')">{{ t('changes.stageAll') }}</Button>
              </div>
            </div>
            <FileList v-if="open.unstaged" :files="unstaged" :mode="ui.prefs.fileView" :active-path="activeSource === 'unstaged' || activeSource === 'untracked' ? activePath : null" @open="(f) => openFile(f, 'unstaged')" @ctx="(files: FileChange[]) => setMenu(files, 'unstaged')">
              <template #actions="{ selection }">
                <Button size="icon-xs" variant="ghost" title="Descartar" class="hover:text-destructive" @click="actions.discard(selection)"><Trash2 /></Button>
                <Button size="icon-xs" variant="ghost" title="Preparar" class="hover:text-success" @click="actions.stage(selection)"><Plus /></Button>
              </template>
            </FileList>
            <p v-if="!unstaged.length && open.unstaged" class="px-4 py-3 text-[12px] text-muted-foreground">Nenhuma alteração não preparada.</p>
          </section>

          <!-- staged -->
          <section>
            <div class="sticky top-0 z-10 flex h-8 items-center gap-1 border-y bg-panel px-2">
              <button class="flex items-center gap-1 text-[12.5px] font-semibold" @click="open.staged = !open.staged">
                <component :is="open.staged ? ChevronDown : ChevronRight" class="size-4" /> {{ t('changes.staged') }} ({{ staged.length }})
              </button>
              <Button v-if="staged.length" size="xs" variant="outline" class="ml-auto" @click="actions.unstage('all')">{{ t('changes.unstageAll') }}</Button>
            </div>
            <FileList v-if="open.staged" :files="staged" :mode="ui.prefs.fileView" :active-path="activeSource === 'staged' ? activePath : null" @open="(f) => openFile(f, 'staged')" @ctx="(files: FileChange[]) => setMenu(files, 'staged')">
              <template #actions="{ selection }">
                <Button size="icon-xs" variant="ghost" title="Remover do stage" @click="actions.unstage(selection)"><Minus /></Button>
              </template>
            </FileList>
            <p v-if="!staged.length && open.staged" class="px-4 py-3 text-[12px] text-muted-foreground">Prepare arquivos (stage) para incluí-los no próximo commit.</p>
          </section>
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent class="w-64">
        <template v-if="menu">
        <FileMenuItems :files="menu.files" :repo="repo" :actions="actions" :source="menu.source" />
              </template>
        <ContextMenuItem v-else disabled>Nenhuma ação aqui</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>

    <!-- composer -->
    <div class="shrink-0 border-t bg-panel-2 p-3" @keydown="onKey">
      <div class="mb-2 flex items-center gap-2">
        <GitCommitHorizontal class="size-4 text-muted-foreground" />
        <span class="text-[12.5px] font-semibold">{{ repo.composer.amend ? 'Amend' : 'Commit' }}</span>
        <button class="ml-auto flex max-w-[60%] items-center gap-1 truncate text-[11.5px] text-muted-foreground hover:text-foreground" title="Identidade Git usada nos commits" @click="editIdentity">
          <UserCog class="size-3.5 shrink-0" />
          <span v-if="identity?.effectiveName" class="truncate">{{ identity.effectiveName }} &lt;{{ identity.effectiveEmail }}&gt;</span>
          <span v-else class="text-warning">Configurar identidade</span>
        </button>
      </div>
      <div class="relative">
        <input
          v-model="repo.composer.summary"
          class="h-8 w-full rounded-md border border-input bg-background pr-10 pl-2.5 text-[13px] outline-none focus:border-ring"
          :placeholder="t('changes.summary')"
          maxlength="200"
        />
        <span class="absolute top-1/2 right-2 -translate-y-1/2 text-[11px] tabular-nums" :class="summaryLen > 72 ? 'text-warning' : 'text-faint'">{{ summaryLen }}</span>
      </div>
      <textarea
        v-model="repo.composer.description"
        rows="3"
        class="mt-1.5 w-full resize-y rounded-md border border-input bg-background px-2.5 py-1.5 text-[12.5px] outline-none focus:border-ring"
        :placeholder="t('changes.description')"
      />
      <div class="mt-1.5 flex items-center gap-3 text-[12px]">
        <label class="flex items-center gap-1.5"><Checkbox :model-value="repo.composer.amend" @update:model-value="(v) => actions.toggleAmend(v === true)" /> Amend</label>
        <button class="flex items-center gap-0.5 text-muted-foreground hover:text-foreground" @click="showOptions = !showOptions">
          <component :is="showOptions ? ChevronDown : ChevronRight" class="size-3.5" /> Opções
        </button>
      </div>
      <div v-if="showOptions" class="mt-1.5 grid gap-1 rounded-md border bg-background/40 p-2 text-[12px]">
        <label class="flex items-center gap-1.5"><Checkbox v-model="repo.composer.signOff" /> Adicionar Signed-off-by</label>
        <label class="flex items-center gap-1.5"><Checkbox v-model="repo.composer.noVerify" /> Ignorar hooks (--no-verify)</label>
        <span class="text-[11px] text-muted-foreground">Assinatura GPG/SSH: {{ identity?.gpgSign ? 'ativada pela configuração do Git (commit.gpgsign)' : 'desativada na configuração do Git' }}.</span>
      </div>
      <Button class="mt-2 h-9 w-full" :disabled="!canCommit" @click="doCommit">
        <LoaderCircle v-if="committing" class="animate-spin" />
        <template v-if="conflicted.length">Resolva os conflitos antes do commit</template>
        <template v-else-if="!staged.length && !repo.composer.amend">Prepare alterações para o commit</template>
        <template v-else>{{ repo.composer.amend ? 'Amend do último commit' : `Commit de ${staged.length} arquivo(s) em ${repo.currentBranch ?? 'HEAD'}` }}</template>
      </Button>
      <p class="mt-1 text-center text-[10.5px] text-faint">Ctrl+Enter para fazer commit</p>
    </div>
  </div>
</template>
