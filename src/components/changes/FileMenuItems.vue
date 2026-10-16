<script setup lang="ts">
import { computed } from 'vue'
import { Archive, Copy, ExternalLink, FileSearch, FolderOpen, History, Minus, Plus, ScanText, Trash2 } from '@lucide/vue'
import { ContextMenuItem, ContextMenuSeparator } from '@/components/ui/context-menu'
import { copyText, type Actions } from '@/features/actions'
import { api, toAppError } from '@/services/api'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { FileChange } from '@/types/git'
import FileHistoryDialog from '@/components/dialogs/FileHistoryDialog.vue'

const props = defineProps<{
  files: FileChange[]
  repo: RepoStore
  actions: Actions
  /** Where the files come from: working tree sections or a commit. */
  source: 'unstaged' | 'staged' | 'conflicted' | 'commit'
  rev?: string | null
}>()
const ui = useUiStore()
const single = computed(() => (props.files.length === 1 ? props.files[0] : null))
const sep = '\\'
const fullPath = (p: string) => `${props.repo.path}${sep}${p.replace(/\//g, sep)}`

function openFile() {
  if (single.value) api.openRepoFile(props.repo.path, single.value.path).catch((e) => ui.error(toAppError(e)))
}
function reveal() {
  if (single.value) api.openInExplorer(fullPath(single.value.path)).catch((e) => ui.error(toAppError(e)))
}
function history() {
  if (single.value) void ui.openDialog(FileHistoryDialog, { repo: props.repo, path: single.value.path })
}
function blame() {
  if (single.value) props.repo.openDiff({ kind: 'blame', path: single.value.path, rev: props.rev ?? null })
}
</script>

<template>
  <template v-if="source === 'unstaged'">
    <ContextMenuItem @select="actions.stage(files)"><Plus /> Preparar {{ files.length > 1 ? `${files.length} arquivos` : '' }}</ContextMenuItem>
    <ContextMenuItem @select="actions.stashSave(files.map((f) => f.path))"><Archive /> Stash {{ files.length > 1 ? 'destes arquivos' : 'deste arquivo' }}…</ContextMenuItem>
    <ContextMenuItem variant="destructive" @select="actions.discard(files)"><Trash2 /> Descartar alterações…</ContextMenuItem>
    <ContextMenuSeparator />
  </template>
  <template v-else-if="source === 'staged'">
    <ContextMenuItem @select="actions.unstage(files)"><Minus /> Remover do stage</ContextMenuItem>
    <ContextMenuSeparator />
  </template>
  <template v-else-if="source === 'conflicted'">
    <ContextMenuItem @select="actions.resolve(files, 'ours')">Resolver usando a nossa versão (ours)</ContextMenuItem>
    <ContextMenuItem @select="actions.resolve(files, 'theirs')">Resolver usando a versão deles (theirs)</ContextMenuItem>
    <ContextMenuItem @select="actions.resolve(files, 'mark_resolved')">Marcar como resolvido</ContextMenuItem>
    <ContextMenuSeparator />
  </template>
  <template v-if="single">
    <ContextMenuItem v-if="single.status !== 'deleted'" @select="openFile"><ExternalLink /> Abrir no app padrão / editor</ContextMenuItem>
    <ContextMenuItem @select="reveal"><FolderOpen /> Mostrar no Explorer</ContextMenuItem>
    <ContextMenuItem @select="history"><History /> Histórico do arquivo</ContextMenuItem>
    <ContextMenuItem v-if="single.status !== 'untracked' && single.status !== 'added' && single.status !== 'deleted'" @select="blame"><ScanText /> Blame</ContextMenuItem>
    <ContextMenuSeparator />
    <ContextMenuItem @select="copyText(single.path, 'Caminho copiado.')"><Copy /> Copiar caminho relativo</ContextMenuItem>
    <ContextMenuItem @select="copyText(fullPath(single.path), 'Caminho copiado.')"><FileSearch /> Copiar caminho completo</ContextMenuItem>
  </template>
  <ContextMenuItem v-else @select="copyText(files.map((f) => f.path).join('\n'), 'Caminhos copiados.')"><Copy /> Copiar caminhos</ContextMenuItem>
</template>
