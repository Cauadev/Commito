<script setup lang="ts">
import { computed } from 'vue'
import { ArrowRight, GitCompareArrows, X } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import FileList from '@/components/changes/FileList.vue'
import { shortSha } from '@/lib/format'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { FileChange } from '@/types/git'

const props = defineProps<{ repo: RepoStore }>()
const ui = useUiStore()
const cmp = computed(() => props.repo.compare!)
const label = (h: string) => props.repo.refsByTarget.get(h)?.[0]?.short ?? shortSha(h)
const totals = computed(() => cmp.value.files.reduce((acc, f) => ({ a: acc.a + (f.additions ?? 0), d: acc.d + (f.deletions ?? 0) }), { a: 0, d: 0 }))
function openFile(f: FileChange) {
  props.repo.openDiff({ kind: 'commit', sha: cmp.value.target, base: cmp.value.base, path: f.path, label: `${label(cmp.value.base)} → ${label(cmp.value.target)}` })
}
function swap() {
  props.repo.compare = { base: cmp.value.target, target: cmp.value.base, files: cmp.value.files }
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-panel">
    <div class="flex h-9 shrink-0 items-center gap-2 border-b px-3">
      <GitCompareArrows class="size-4 text-muted-foreground" />
      <span class="text-[12.5px] font-semibold">Comparação</span>
      <Button size="icon-xs" variant="ghost" class="ml-auto" title="Fechar comparação" @click="repo.compare = null; repo.closeDiff()"><X /></Button>
    </div>
    <div class="border-b px-3 py-2.5 text-[12.5px]">
      <div class="flex items-center gap-2">
        <span class="mono rounded bg-muted px-1.5 py-0.5">{{ label(cmp.base) }}</span>
        <ArrowRight class="size-3.5 text-muted-foreground" />
        <span class="mono rounded bg-primary/15 px-1.5 py-0.5 text-primary">{{ label(cmp.target) }}</span>
        <Button size="xs" variant="ghost" class="ml-auto" @click="swap">Inverter</Button>
      </div>
      <p class="mt-1.5 text-[12px] text-muted-foreground">
        {{ cmp.files.length }} arquivo(s) · <span class="text-added">+{{ totals.a }}</span> <span class="text-removed">−{{ totals.d }}</span>
      </p>
    </div>
    <div class="min-h-0 flex-1 overflow-auto">
      <FileList :files="cmp.files" :mode="ui.prefs.fileView" @open="openFile" />
      <p v-if="!cmp.files.length" class="p-4 text-[12px] text-muted-foreground">Sem diferenças.</p>
    </div>
  </div>
</template>
