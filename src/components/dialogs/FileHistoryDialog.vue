<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { History, LoaderCircle } from '@lucide/vue'
import { api, toAppError } from '@/services/api'
import { absoluteTime, relativeTime, shortSha } from '@/lib/format'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { HistoryEntry } from '@/types/git'
import DialogShell from './DialogShell.vue'

const props = defineProps<{ repo: RepoStore; path: string }>()
const ui = useUiStore()
const items = ref<HistoryEntry[]>([])
const loading = ref(true)

onMounted(async () => {
  try {
    items.value = await api.getFileHistory(props.repo.path, props.path)
  } catch (e) {
    ui.error(toAppError(e))
  } finally {
    loading.value = false
  }
})

async function pick(c: HistoryEntry) {
  ui.closeDialog()
  if (await props.repo.ensureLoaded(c.hash, 20000)) props.repo.selectCommit(c.hash)
  props.repo.openDiff({ kind: 'commit', sha: c.hash, base: null, path: props.path, label: c.subject })
}
</script>

<template>
  <DialogShell :title="`Histórico de ${path}`" description="Commits que alteraram este arquivo (seguindo renomeações). Clique para ver o diff." width="sm:max-w-2xl" @close="ui.closeDialog()">
    <div v-if="loading" class="flex items-center gap-2 py-6 text-muted-foreground"><LoaderCircle class="size-4 animate-spin" /> Carregando…</div>
    <p v-else-if="!items.length" class="py-4 text-muted-foreground">Nenhum commit encontrado para este arquivo.</p>
    <div v-else class="max-h-[60vh] overflow-auto rounded-lg border">
      <button
        v-for="c in items"
        :key="c.hash"
        class="flex w-full items-center gap-2 border-b px-3 py-1.5 text-left last:border-b-0 hover:bg-[var(--row-hover)]"
        @click="pick(c)"
      >
        <History class="size-3.5 shrink-0 text-muted-foreground" />
        <span class="mono text-[11.5px] text-primary">{{ shortSha(c.hash) }}</span>
        <span class="min-w-0 flex-1 truncate text-[12.5px]">{{ c.subject }}</span>
        <span class="max-w-32 truncate text-[11.5px] text-muted-foreground">{{ c.authorName }}</span>
        <span class="w-24 shrink-0 text-right text-[11.5px] text-muted-foreground" :title="absoluteTime(c.authorTime)">{{ relativeTime(c.authorTime) }}</span>
      </button>
    </div>
  </DialogShell>
</template>
