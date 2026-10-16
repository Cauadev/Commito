<script setup lang="ts">
import { computed } from 'vue'
import {
  ArrowDownToLine, ArrowUpFromLine, Copy, FolderGit2, GitBranchPlus, GitMerge, GitPullRequestArrow, Link2,
  LogIn, Pencil, Tag, Trash2, Upload,
} from '@lucide/vue'
import { ContextMenuItem, ContextMenuSeparator } from '@/components/ui/context-menu'
import { copyText, type Actions } from '@/features/actions'
import type { RepoStore } from '@/stores/repo'
import type { GitRef } from '@/types/git'

const props = defineProps<{ target: GitRef; repo: RepoStore; actions: Actions }>()
const r = computed(() => props.target)
const cur = computed(() => props.repo.currentBranch)
const a = props.actions
</script>

<template>
  <template v-if="r.kind === 'local'">
    <ContextMenuItem :disabled="r.isHead" @select="a.checkoutRef(r)"><LogIn /> Checkout de {{ r.short }}</ContextMenuItem>
    <template v-if="r.isHead">
      <ContextMenuItem @select="a.pull()"><ArrowDownToLine /> Pull</ContextMenuItem>
      <ContextMenuItem @select="a.push()"><ArrowUpFromLine /> Push</ContextMenuItem>
    </template>
    <template v-else>
      <ContextMenuItem :disabled="!cur" @select="a.mergeInto(r.short)"><GitMerge /> Merge de {{ r.short }} em {{ cur ?? 'HEAD' }}</ContextMenuItem>
      <ContextMenuItem :disabled="!cur" @select="a.rebaseOnto(r.short, r.short)"><GitPullRequestArrow /> Rebase de {{ cur ?? 'HEAD' }} sobre {{ r.short }}</ContextMenuItem>
      <ContextMenuItem @select="a.push({ branch: r.short })"><Upload /> Push de {{ r.short }}</ContextMenuItem>
    </template>
    <ContextMenuSeparator />
    <ContextMenuItem @select="a.createBranch(r.short, r.short)"><GitBranchPlus /> Criar branch a partir daqui…</ContextMenuItem>
    <ContextMenuItem @select="a.createWorktree(r.short)"><FolderGit2 /> Criar worktree para {{ r.short }}…</ContextMenuItem>
    <ContextMenuItem @select="a.createTag(r.target, r.short)"><Tag /> Criar tag aqui…</ContextMenuItem>
    <ContextMenuItem @select="a.setUpstream(r.short)"><Link2 /> Definir upstream…</ContextMenuItem>
    <ContextMenuSeparator />
    <ContextMenuItem @select="a.renameBranch(r)"><Pencil /> Renomear…</ContextMenuItem>
    <ContextMenuItem :disabled="r.isHead" variant="destructive" @select="a.deleteBranch(r)"><Trash2 /> Excluir {{ r.short }}</ContextMenuItem>
    <ContextMenuSeparator />
    <ContextMenuItem @select="copyText(r.short)"><Copy /> Copiar nome</ContextMenuItem>
  </template>

  <template v-else-if="r.kind === 'remote'">
    <ContextMenuItem @select="a.checkoutRef(r)"><LogIn /> Checkout (branch local rastreando {{ r.short }})</ContextMenuItem>
    <ContextMenuItem :disabled="!cur" @select="a.mergeInto(r.short)"><GitMerge /> Merge de {{ r.short }} em {{ cur ?? 'HEAD' }}</ContextMenuItem>
    <ContextMenuItem :disabled="!cur" @select="a.rebaseOnto(r.short, r.short)"><GitPullRequestArrow /> Rebase de {{ cur ?? 'HEAD' }} sobre {{ r.short }}</ContextMenuItem>
    <ContextMenuSeparator />
    <ContextMenuItem @select="a.createBranch(r.short, r.short)"><GitBranchPlus /> Criar branch a partir daqui…</ContextMenuItem>
    <ContextMenuItem @select="a.fetchAll(r.remote ?? null)"><ArrowDownToLine /> Fetch {{ r.remote }}</ContextMenuItem>
    <ContextMenuSeparator />
    <ContextMenuItem variant="destructive" @select="a.deleteRemoteBranch(r)"><Trash2 /> Excluir {{ r.short }} do remote…</ContextMenuItem>
    <ContextMenuItem @select="copyText(r.short)"><Copy /> Copiar nome</ContextMenuItem>
  </template>

  <template v-else>
    <ContextMenuItem @select="a.checkoutCommit(r.target, r.short)"><LogIn /> Checkout da tag {{ r.short }}</ContextMenuItem>
    <ContextMenuItem @select="a.createBranch(r.target, r.short)"><GitBranchPlus /> Criar branch a partir da tag…</ContextMenuItem>
    <ContextMenuItem :disabled="!repo.remotes.length" @select="a.pushTag(r)"><Upload /> Enviar tag para {{ a.defaultRemote() ?? 'remote' }}</ContextMenuItem>
    <ContextMenuSeparator />
    <ContextMenuItem @select="a.renameTag(r)"><Pencil /> Renomear…</ContextMenuItem>
    <ContextMenuItem variant="destructive" @select="a.deleteTag(r)"><Trash2 /> Excluir tag…</ContextMenuItem>
    <ContextMenuItem @select="copyText(r.short)"><Copy /> Copiar nome</ContextMenuItem>
  </template>
</template>
