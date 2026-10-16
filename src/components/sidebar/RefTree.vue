<script setup lang="ts">
// Branch names grouped into folders by "/" (feature/x, feature/y -> feature/).
import { computed, ref } from 'vue'
import { ArrowDown, ArrowUp, Check, ChevronDown, ChevronRight, CloudOff, Folder, FolderGit2, GitBranch } from '@lucide/vue'
import type { GitRef } from '@/types/git'

const props = defineProps<{ refs: GitRef[]; strip?: string; depth?: number; selectedTarget?: string | null }>()
const emit = defineEmits<{ select: [r: GitRef]; open: [r: GitRef]; ctx: [r: GitRef] }>()

interface Group { name: string; children: GitRef[]; folders: Map<string, GitRef[]> }
const collapsed = ref<Set<string>>(new Set())

const group = computed<Group>(() => {
  const g: Group = { name: '', children: [], folders: new Map() }
  for (const r of props.refs) {
    const name = props.strip ? r.short.slice(props.strip.length) : r.short
    const slash = name.indexOf('/')
    if (slash > 0) {
      const folder = name.slice(0, slash)
      const list = g.folders.get(folder) ?? []
      list.push(r)
      g.folders.set(folder, list)
    } else g.children.push(r)
  }
  return g
})
const displayName = (r: GitRef, folder?: string) => {
  const n = props.strip ? r.short.slice(props.strip.length) : r.short
  return folder ? n.slice(folder.length + 1) : n
}
function toggle(f: string) {
  const s = new Set(collapsed.value)
  if (s.has(f)) s.delete(f)
  else s.add(f)
  collapsed.value = s
}
const pad = (extra = 0) => ({ paddingLeft: 10 + ((props.depth ?? 0) + extra) * 14 + 'px' })
</script>

<template>
  <div>
    <template v-for="[folder, items] in [...group.folders.entries()].sort((a, b) => a[0].localeCompare(b[0]))" :key="folder">
      <div class="flex h-6 cursor-default items-center gap-1 pr-2 text-[12.5px] text-muted-foreground hover:bg-[var(--row-hover)]" :style="pad()" @click="toggle(folder)">
        <component :is="collapsed.has(folder) ? ChevronRight : ChevronDown" class="size-3.5 shrink-0" />
        <Folder class="size-3.5 shrink-0 opacity-70" />
        <span class="truncate">{{ folder }}</span>
        <span class="ml-auto text-[11px] text-faint">{{ items.length }}</span>
      </div>
      <template v-if="!collapsed.has(folder)">
        <div
          v-for="r in items"
          :key="r.name"
          class="flex h-6 cursor-default items-center gap-1.5 pr-2 text-[12.5px]"
          :class="selectedTarget === r.target ? 'bg-[var(--row-selected)]' : 'hover:bg-[var(--row-hover)]'"
          :style="pad(1.3)"
          @click="emit('select', r)"
          @dblclick="emit('open', r)"
          @contextmenu="emit('ctx', r)"
        >
          <Check v-if="r.isHead" class="size-3.5 shrink-0 text-primary" />
          <GitBranch v-else class="size-3.5 shrink-0 text-muted-foreground" />
          <span class="truncate" :class="r.isHead ? 'font-semibold' : ''">{{ displayName(r, folder) }}</span>
          <span class="ml-auto flex shrink-0 items-center gap-1 text-[10.5px] text-muted-foreground">
            <FolderGit2 v-if="r.worktreePath && !r.isHead" class="size-3 text-warning" />
            <CloudOff v-if="r.upstreamGone" class="size-3 text-warning" />
            <span v-if="r.ahead" class="flex items-center text-info"><ArrowUp class="size-3" />{{ r.ahead }}</span>
            <span v-if="r.behind" class="flex items-center text-warning"><ArrowDown class="size-3" />{{ r.behind }}</span>
          </span>
        </div>
      </template>
    </template>
    <div
      v-for="r in group.children"
      :key="r.name"
      class="flex h-6 cursor-default items-center gap-1.5 pr-2 text-[12.5px]"
      :class="selectedTarget === r.target ? 'bg-[var(--row-selected)]' : 'hover:bg-[var(--row-hover)]'"
      :style="pad(0.3)"
      @click="emit('select', r)"
      @dblclick="emit('open', r)"
      @contextmenu="emit('ctx', r)"
    >
      <Check v-if="r.isHead" class="size-3.5 shrink-0 text-primary" />
      <GitBranch v-else class="size-3.5 shrink-0 text-muted-foreground" />
      <span class="truncate" :class="r.isHead ? 'font-semibold' : ''">{{ displayName(r) }}</span>
      <span class="ml-auto flex shrink-0 items-center gap-1 text-[10.5px] text-muted-foreground">
        <FolderGit2 v-if="r.worktreePath && !r.isHead" class="size-3 text-warning" />
        <CloudOff v-if="r.upstreamGone" class="size-3 text-warning" title="Upstream removido" />
        <span v-if="r.ahead" class="flex items-center text-info"><ArrowUp class="size-3" />{{ r.ahead }}</span>
        <span v-if="r.behind" class="flex items-center text-warning"><ArrowDown class="size-3" />{{ r.behind }}</span>
      </span>
    </div>
  </div>
</template>
