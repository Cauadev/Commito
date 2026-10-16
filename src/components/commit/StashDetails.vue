<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Archive, ArchiveRestore, GitBranchPlus, LoaderCircle, Trash2 } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import FileList from '@/components/changes/FileList.vue'
import { useActions } from '@/features/actions'
import { api } from '@/services/api'
import { absoluteTime, relativeTime, shortSha } from '@/lib/format'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { CommitDetail, FileChange } from '@/types/git'

const props = defineProps<{ repo: RepoStore; hash: string }>()
const repo = props.repo
const ui = useUiStore()
const actions = useActions(repo)
const detail = ref<CommitDetail | null>(null)
const untracked = ref<FileChange[]>([])
const loading = ref(false)
const stash = computed(() => repo.stashes.find((s) => s.hash === props.hash) ?? null)

watch(
  () => props.hash,
  async (h) => {
    loading.value = true
    detail.value = await repo.commitDetail(h)
    untracked.value = await api.getStashUntracked(repo.path, h).catch(() => [])
    loading.value = false
  },
  { immediate: true },
)

function openFile(f: FileChange) {
  const base = f.status === 'untracked' ? null : (detail.value?.parents[0] ?? null)
  const sha = f.status === 'untracked' ? `${props.hash}^3` : props.hash
  repo.openDiff({ kind: 'commit', sha, base, path: f.path, label: stash.value?.message })
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-panel">
    <div class="flex h-9 shrink-0 items-center gap-2 border-b px-3">
      <Archive class="size-4 text-muted-foreground" />
      <span class="text-[12.5px] font-semibold">{{ stash?.reference ?? 'Stash' }}</span>
      <LoaderCircle v-if="loading" class="size-3.5 animate-spin text-muted-foreground" />
    </div>
    <div v-if="stash" class="border-b px-3 py-3">
      <h2 class="text-[14px] font-semibold">{{ stash.message }}</h2>
      <p class="mt-1 text-[12px] text-muted-foreground">
        Criado {{ relativeTime(stash.time) }} ({{ absoluteTime(stash.time) }}) {{ stash.branch ? `em ${stash.branch}` : '' }}, sobre
        <span class="mono">{{ shortSha(stash.base) }}</span>
      </p>
      <div class="mt-3 flex flex-wrap gap-1.5">
        <Button size="xs" @click="actions.stashApply(stash, true)"><ArchiveRestore /> Pop</Button>
        <Button size="xs" variant="outline" @click="actions.stashApply(stash, false)">Aplicar</Button>
        <Button size="xs" variant="outline" @click="actions.stashBranch(stash)"><GitBranchPlus /> Branch</Button>
        <Button size="xs" variant="destructive" @click="actions.stashDrop(stash)"><Trash2 /> Descartar</Button>
      </div>
    </div>
    <div class="min-h-0 flex-1 overflow-auto">
      <div class="flex h-8 items-center border-b px-3 text-[12px] font-semibold">Arquivos rastreados ({{ detail?.files.length ?? 0 }})</div>
      <FileList v-if="detail" :files="detail.files" :mode="ui.prefs.fileView" @open="openFile" />
      <template v-if="untracked.length">
        <div class="flex h-8 items-center border-y px-3 text-[12px] font-semibold">Não rastreados ({{ untracked.length }})</div>
        <FileList :files="untracked" :mode="ui.prefs.fileView" @open="openFile" />
      </template>
    </div>
  </div>
</template>
