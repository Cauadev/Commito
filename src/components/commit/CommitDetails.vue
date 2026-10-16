<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { CherryIcon, Copy, FolderTree, GitBranchPlus, List, LoaderCircle, LogIn, ShieldCheck, ShieldAlert, Undo2 } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@/components/ui/context-menu'
import FileList from '@/components/changes/FileList.vue'
import FileMenuItems from '@/components/changes/FileMenuItems.vue'
import RefBadges from '@/components/graph/RefBadges.vue'
import { copyText, useActions } from '@/features/actions'
import { absoluteTime, hashHue, initials, relativeTime, shortSha } from '@/lib/format'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { CommitDetail, FileChange } from '@/types/git'

const props = defineProps<{ repo: RepoStore; hash: string }>()
const repo = props.repo
const ui = useUiStore()
const actions = useActions(repo)
const detail = ref<CommitDetail | null>(null)
const loading = ref(false)

watch(
  () => props.hash,
  async (h) => {
    loading.value = true
    detail.value = null
    const d = await repo.commitDetail(h)
    if (props.hash === h) detail.value = d
    loading.value = false
  },
  { immediate: true },
)

const row = computed(() => repo.rows[repo.rowIndex.get(props.hash) ?? -1])
const refs = computed(() => repo.refsByTarget.get(props.hash) ?? [])
const activePath = computed(() => (repo.diffTarget?.kind === 'commit' && repo.diffTarget.sha === props.hash ? repo.diffTarget.path : null))
const totals = computed(() => {
  let a = 0
  let d = 0
  for (const f of detail.value?.files ?? []) {
    a += f.additions ?? 0
    d += f.deletions ?? 0
  }
  return { a, d }
})
const sameCommitter = computed(() => detail.value && detail.value.committerName === detail.value.authorName && detail.value.committerEmail === detail.value.authorEmail)

function openFile(f: FileChange) {
  repo.openDiff({ kind: 'commit', sha: props.hash, base: null, path: f.path, label: detail.value?.subject })
}
function goParent(p: string) {
  if (repo.rowIndex.has(p)) repo.selectCommit(p)
  else void repo.ensureLoaded(p).then((ok) => ok && repo.selectCommit(p))
}
const menu = ref<FileChange[] | null>(null)
const sig = computed(() => {
  const s = detail.value?.signature
  if (!s) return null
  return s === 'good' ? { ok: true, text: 'Assinatura válida' } : { ok: false, text: `Assinatura: ${s}` }
})
const avatar = (n: string) => ({ background: `oklch(0.62 0.12 ${hashHue(n)})` })
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-panel">
    <div class="flex h-9 shrink-0 items-center gap-2 border-b px-3">
      <span class="text-[12.5px] text-muted-foreground">Commit</span>
      <button class="mono rounded bg-primary/15 px-1.5 py-0.5 text-[12px] text-primary hover:bg-primary/25" title="Copiar SHA completo" @click="copyText(hash, 'SHA copiado.')">{{ shortSha(hash, 10) }}</button>
      <LoaderCircle v-if="loading" class="size-3.5 animate-spin text-muted-foreground" />
      <div class="ml-auto flex rounded-md border p-0.5">
        <button class="rounded px-1 py-0.5" :class="ui.prefs.fileView === 'path' ? 'bg-muted' : 'text-muted-foreground'" title="Path" @click="ui.prefs.fileView = 'path'"><List class="size-3.5" /></button>
        <button class="rounded px-1 py-0.5" :class="ui.prefs.fileView === 'tree' ? 'bg-muted' : 'text-muted-foreground'" title="Tree" @click="ui.prefs.fileView = 'tree'"><FolderTree class="size-3.5" /></button>
      </div>
    </div>

    <div v-if="detail" class="min-h-0 flex-1 overflow-auto">
      <div class="border-b px-3 py-3">
        <div v-if="refs.length" class="mb-2"><RefBadges :refs="refs" :color="row?.color ?? 0" :head-branch="repo.currentBranch" :max="6" /></div>
        <h2 class="selectable text-[14px] leading-snug font-semibold">{{ detail.subject }}</h2>
        <pre v-if="detail.body" class="selectable mt-2 font-sans text-[12.5px] leading-relaxed whitespace-pre-wrap text-foreground/85">{{ detail.body }}</pre>

        <div class="mt-3 flex items-center gap-2">
          <span class="grid size-7 shrink-0 place-items-center rounded-full text-[10px] font-bold text-black/80" :style="avatar(detail.authorName)">{{ initials(detail.authorName) }}</span>
          <div class="min-w-0 text-[12px] leading-tight">
            <div class="truncate"><b>{{ detail.authorName }}</b> <span class="text-muted-foreground">&lt;{{ detail.authorEmail }}&gt;</span></div>
            <div class="text-muted-foreground" :title="absoluteTime(detail.authorTime)">criou {{ relativeTime(detail.authorTime) }} · {{ absoluteTime(detail.authorTime) }}</div>
            <div v-if="!sameCommitter" class="truncate text-muted-foreground">commit por {{ detail.committerName }} {{ relativeTime(detail.committerTime) }}</div>
          </div>
        </div>
        <div class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11.5px] text-muted-foreground">
          <span>
            {{ detail.parents.length > 1 ? 'pais' : 'pai' }}:
            <button v-for="p in detail.parents" :key="p" class="mono ml-1 text-primary hover:underline" @click="goParent(p)">{{ shortSha(p) }}</button>
            <span v-if="!detail.parents.length">(commit raiz)</span>
          </span>
          <span v-if="sig" class="flex items-center gap-1" :class="sig.ok ? 'text-success' : 'text-warning'">
            <component :is="sig.ok ? ShieldCheck : ShieldAlert" class="size-3.5" /> {{ sig.text }}
          </span>
        </div>
        <div class="mt-3 flex flex-wrap gap-1.5">
          <Button size="xs" variant="outline" @click="actions.cherryPick([hash])"><CherryIcon /> Cherry-pick</Button>
          <Button size="xs" variant="outline" @click="actions.createBranch(hash, shortSha(hash))"><GitBranchPlus /> Branch</Button>
          <Button size="xs" variant="outline" @click="actions.revert(hash)"><Undo2 /> Revert</Button>
          <Button size="xs" variant="outline" @click="actions.checkoutCommit(hash)"><LogIn /> Checkout</Button>
          <Button size="xs" variant="ghost" @click="copyText(hash, 'SHA copiado.')"><Copy /> SHA</Button>
        </div>
      </div>

      <div class="flex h-8 items-center gap-2 border-b px-3 text-[12px]">
        <span class="font-semibold">{{ detail.files.length }} arquivo(s)</span>
        <span class="mono text-added">+{{ totals.a }}</span>
        <span class="mono text-removed">−{{ totals.d }}</span>
        <span v-if="detail.parents.length > 1" class="ml-auto text-[11px] text-muted-foreground">diff contra o 1º pai</span>
      </div>
      <ContextMenu>
        <ContextMenuTrigger as-child>
          <div>
            <FileList :files="detail.files" :mode="ui.prefs.fileView" :active-path="activePath" @open="openFile" @ctx="(f: FileChange[]) => (menu = f)" />
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent class="w-60">
        <template v-if="menu">
          <FileMenuItems :files="menu" :repo="repo" :actions="actions" source="commit" :rev="hash" />
                </template>
        <ContextMenuItem v-else disabled>Nenhuma ação aqui</ContextMenuItem>
      </ContextMenuContent>
      </ContextMenu>
    </div>
  </div>
</template>
