<script setup lang="ts">
import { emojify } from '@/lib/emoji'
import { computed } from 'vue'
import { CherryIcon, Copy, GitCompareArrows } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import ChangesPanel from '@/components/changes/ChangesPanel.vue'
import CommitDetails from './CommitDetails.vue'
import StashDetails from './StashDetails.vue'
import ComparePanel from './ComparePanel.vue'
import { copyText, useActions } from '@/features/actions'
import { api, toAppError } from '@/services/api'
import { shortSha } from '@/lib/format'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'

const props = defineProps<{ repo: RepoStore }>()
const repo = props.repo
const ui = useUiStore()
const actions = useActions(repo)
const sel = computed(() => repo.selection)
const multi = computed(() => (sel.value.kind === 'commits' && sel.value.hashes.length > 1 ? sel.value.hashes : null))

async function compareEnds() {
  const hs = multi.value!
  const sorted = [...hs].sort((a, b) => (repo.rowIndex.get(b) ?? 0) - (repo.rowIndex.get(a) ?? 0))
  try {
    const files = await api.compareRefs(repo.path, sorted[0], sorted[sorted.length - 1])
    repo.compare = { base: sorted[0], target: sorted[sorted.length - 1], files }
  } catch (e) {
    ui.error(toAppError(e))
  }
}
</script>

<template>
  <ComparePanel v-if="repo.compare" :repo="repo" />
  <ChangesPanel v-else-if="sel.kind === 'wip'" :repo="repo" />
  <StashDetails v-else-if="sel.kind === 'stash'" :repo="repo" :hash="sel.hash" />
  <div v-else-if="multi" class="flex h-full flex-col bg-panel">
    <div class="flex h-9 items-center border-b px-3 text-[12.5px] font-semibold">{{ multi.length }} commits selecionados</div>
    <div class="min-h-0 flex-1 overflow-auto">
      <div v-for="h in multi" :key="h" class="flex items-center gap-2 border-b px-3 py-1.5 text-[12.5px]">
        <span class="mono text-[11.5px] text-primary">{{ shortSha(h) }}</span>
        <span class="truncate">{{ emojify(repo.rows[repo.rowIndex.get(h) ?? -1]?.subject ?? '') }}</span>
      </div>
    </div>
    <div class="grid gap-1.5 border-t p-3">
      <Button @click="actions.cherryPick(multi)"><CherryIcon /> Cherry-pick de {{ multi.length }} commits em {{ repo.currentBranch ?? 'HEAD' }}</Button>
      <Button variant="outline" @click="compareEnds"><GitCompareArrows /> Comparar mais antigo → mais recente</Button>
      <Button variant="ghost" @click="copyText(multi.join('\n'), 'SHAs copiados.')"><Copy /> Copiar SHAs</Button>
      <p class="text-[11px] text-muted-foreground">Ctrl+clique adiciona/remove commits; Shift+clique seleciona um intervalo.</p>
    </div>
  </div>
  <CommitDetails v-else-if="sel.kind === 'commits'" :repo="repo" :hash="sel.hashes[0]" />
</template>
