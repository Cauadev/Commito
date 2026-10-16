<script setup lang="ts">
import { computed } from 'vue'
import {
  CherryIcon, Copy, ExternalLink, GitBranchPlus, GitCompareArrows, GitMerge, GitPullRequestArrow, ListRestart,
  LogIn, RotateCcw, Tag, Undo2, Rows3,
} from '@lucide/vue'
import {
  ContextMenuItem, ContextMenuSeparator, ContextMenuSub, ContextMenuSubContent, ContextMenuSubTrigger,
} from '@/components/ui/context-menu'
import { copyText, type Actions } from '@/features/actions'
import { shortSha } from '@/lib/format'
import type { RepoStore } from '@/stores/repo'
import type { GraphRow } from '@/types/git'

const props = defineProps<{ row: GraphRow; repo: RepoStore; actions: Actions }>()
const emit = defineEmits<{ compare: [base: string, target: string] }>()
const a = props.actions
const sel = computed(() => (props.repo.selection.kind === 'commits' ? props.repo.selection.hashes : []))
const multi = computed(() => sel.value.length > 1 && sel.value.includes(props.row.hash))
const cur = computed(() => props.repo.currentBranch)
const isHead = computed(() => props.repo.head?.oid === props.row.hash)
const other = computed(() => sel.value.find((h) => h !== props.row.hash) ?? null)
const hasWeb = computed(() => props.repo.remotes.some((r) => r.webUrl))
</script>

<template>
  <template v-if="multi">
    <ContextMenuItem @select="a.cherryPick(sel)"><CherryIcon /> Cherry-pick de {{ sel.length }} commits em {{ cur ?? 'HEAD' }}</ContextMenuItem>
    <ContextMenuItem v-if="sel.length === 2" @select="emit('compare', sel[1], sel[0])"><GitCompareArrows /> Comparar os 2 commits</ContextMenuItem>
    <ContextMenuItem @select="copyText(sel.join('\n'), 'SHAs copiados.')"><Copy /> Copiar SHAs</ContextMenuItem>
  </template>
  <template v-else>
    <ContextMenuItem :disabled="isHead" @select="a.cherryPick([row.hash])"><CherryIcon /> Cherry-pick em {{ cur ?? 'HEAD' }}</ContextMenuItem>
    <ContextMenuItem @select="a.checkoutCommit(row.hash)"><LogIn /> Checkout deste commit (detached)</ContextMenuItem>
    <ContextMenuItem @select="a.createBranch(row.hash, shortSha(row.hash))"><GitBranchPlus /> Criar branch aqui…</ContextMenuItem>
    <ContextMenuItem @select="a.createTag(row.hash, shortSha(row.hash))"><Tag /> Criar tag aqui…</ContextMenuItem>
    <ContextMenuSeparator />
    <ContextMenuItem :disabled="!cur || isHead" @select="a.mergeInto(row.hash, shortSha(row.hash))"><GitMerge /> Merge deste commit em {{ cur ?? 'HEAD' }}</ContextMenuItem>
    <ContextMenuItem :disabled="!cur || isHead" @select="a.rebaseOnto(row.hash)"><GitPullRequestArrow /> Rebase de {{ cur ?? 'HEAD' }} sobre este commit</ContextMenuItem>
    <ContextMenuItem :disabled="!cur || isHead" @select="a.interactiveRebase(row.hash)"><Rows3 /> Rebase interativo a partir daqui…</ContextMenuItem>
    <ContextMenuItem @select="a.revert(row.hash)"><Undo2 /> Reverter commit…</ContextMenuItem>
    <ContextMenuSub>
      <ContextMenuSubTrigger :disabled="!cur && !repo.head?.detached"><ListRestart /> Reset {{ cur ?? 'HEAD' }} para cá</ContextMenuSubTrigger>
      <ContextMenuSubContent>
        <ContextMenuItem @select="a.reset(row.hash, 'soft')">Soft — mantém alterações preparadas</ContextMenuItem>
        <ContextMenuItem @select="a.reset(row.hash, 'mixed')">Mixed — mantém alterações no working dir</ContextMenuItem>
        <ContextMenuItem variant="destructive" @select="a.reset(row.hash, 'hard')"><RotateCcw /> Hard — descarta alterações…</ContextMenuItem>
      </ContextMenuSubContent>
    </ContextMenuSub>
    <ContextMenuSeparator />
    <ContextMenuItem v-if="!isHead && repo.head?.oid" @select="emit('compare', row.hash, repo.head!.oid!)"><GitCompareArrows /> Comparar com HEAD</ContextMenuItem>
    <ContextMenuItem v-if="other" @select="emit('compare', other!, row.hash)"><GitCompareArrows /> Comparar com {{ shortSha(other) }}</ContextMenuItem>
    <ContextMenuItem @select="copyText(row.hash, 'SHA copiado.')"><Copy /> Copiar SHA</ContextMenuItem>
    <ContextMenuItem @select="copyText(row.subject, 'Mensagem copiada.')"><Copy /> Copiar mensagem</ContextMenuItem>
    <ContextMenuItem v-if="hasWeb" @select="a.openCommitWeb(row.hash)"><ExternalLink /> Abrir commit no navegador</ContextMenuItem>
  </template>
</template>
