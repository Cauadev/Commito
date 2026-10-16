<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ArrowLeft, LoaderCircle } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { api, toAppError } from '@/services/api'
import { highlightLine, languageFor } from '@/lib/highlight'
import { hashHue, relativeTime, shortSha } from '@/lib/format'
import type { RepoStore } from '@/stores/repo'
import type { AppError, Blame } from '@/types/git'

const props = defineProps<{ repo: RepoStore; path: string; rev: string | null }>()
const blame = ref<Blame | null>(null)
const loading = ref(false)
const error = ref<AppError | null>(null)

watch(
  () => [props.path, props.rev],
  async () => {
    loading.value = true
    error.value = null
    try {
      blame.value = await api.getBlame(props.repo.path, props.path, props.rev)
    } catch (e) {
      error.value = toAppError(e)
    } finally {
      loading.value = false
    }
  },
  { immediate: true },
)

const lang = computed(() => languageFor(props.path))
const lines = computed(() => {
  const b = blame.value
  if (!b) return []
  return b.lines.map((l, i) => {
    const prev = b.lines[i - 1]
    const c = b.commits[l.hash]
    return { ...l, first: !prev || prev.hash !== l.hash, commit: c, html: highlightLine(l.content, lang.value) }
  })
})

function select(hash: string) {
  void props.repo.ensureLoaded(hash, 20000).then((ok) => ok && props.repo.selectCommit(hash))
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-background">
    <div class="flex h-10 shrink-0 items-center gap-2 border-b bg-panel px-2">
      <Button size="icon-sm" variant="ghost" title="Voltar (Esc)" @click="repo.closeDiff()"><ArrowLeft /></Button>
      <span class="font-mono text-[12.5px]">{{ path }}</span>
      <span class="text-[11.5px] text-muted-foreground">Blame {{ rev ? `em ${shortSha(rev)}` : '(HEAD + working dir)' }}</span>
      <LoaderCircle v-if="loading" class="size-4 animate-spin text-muted-foreground" />
    </div>
    <div v-if="error" class="m-4 text-[12.5px] text-destructive">{{ error.message }}</div>
    <div v-else class="selectable min-h-0 flex-1 overflow-auto font-mono text-[12px] leading-[19px]">
      <div v-for="l in lines" :key="l.line" class="flex" :class="l.first ? 'border-t border-border/60' : ''">
        <button
          class="flex w-64 shrink-0 items-center gap-2 overflow-hidden border-r px-2 text-left font-sans text-[11px] text-muted-foreground hover:bg-[var(--row-hover)]"
          :style="{ boxShadow: `inset 3px 0 0 oklch(0.65 0.12 ${hashHue(l.hash)})` }"
          :title="l.commit ? `${l.commit.summary}\n${l.commit.author}` : ''"
          @click="select(l.hash)"
        >
          <template v-if="l.first && l.commit">
            <span class="mono text-primary">{{ shortSha(l.hash) }}</span>
            <span class="truncate">{{ l.commit.author }}</span>
            <span class="ml-auto shrink-0">{{ relativeTime(l.commit.time) }}</span>
          </template>
        </button>
        <span class="w-12 shrink-0 pr-2 text-right text-faint select-none">{{ l.line }}</span>
        <span class="flex-1 whitespace-pre pr-4" v-html="l.html || ' '" />
      </div>
    </div>
  </div>
</template>
