<script setup lang="ts">
// File list with Path / Tree modes, multi-selection (Ctrl/Shift) and per-row hover actions.
import { computed, ref } from 'vue'
import { ChevronDown, ChevronRight, Folder } from '@lucide/vue'
import type { FileChange } from '@/types/git'
import { basename, dirname, statusClass, statusLabel, statusLetter } from '@/lib/format'

const props = defineProps<{
  files: FileChange[]
  mode: 'path' | 'tree'
  activePath?: string | null
  dense?: boolean
  actionsVisible?: boolean
}>()
const emit = defineEmits<{
  open: [file: FileChange]
  ctx: [files: FileChange[], e: MouseEvent]
}>()
defineSlots<{ actions(props: { file: FileChange; selection: FileChange[] }): unknown }>()

const selected = ref<Set<string>>(new Set())
let anchor: string | null = null

function select(f: FileChange, e: MouseEvent) {
  if (e.ctrlKey || e.metaKey) {
    const s = new Set(selected.value)
    if (s.has(f.path)) s.delete(f.path)
    else s.add(f.path)
    selected.value = s
    anchor = f.path
    return
  }
  if (e.shiftKey && anchor) {
    const order = flat.value.map((x) => x.path)
    const a = order.indexOf(anchor)
    const b = order.indexOf(f.path)
    if (a >= 0 && b >= 0) {
      const [lo, hi] = a < b ? [a, b] : [b, a]
      selected.value = new Set(order.slice(lo, hi + 1))
      return
    }
  }
  selected.value = new Set([f.path])
  anchor = f.path
  emit('open', f)
}

function selectionFor(f: FileChange): FileChange[] {
  if (selected.value.has(f.path) && selected.value.size > 1) return props.files.filter((x) => selected.value.has(x.path))
  return [f]
}

function onCtx(f: FileChange, e: MouseEvent) {
  if (!selected.value.has(f.path)) {
    selected.value = new Set([f.path])
    anchor = f.path
  }
  emit('ctx', selectionFor(f), e)
}

// ---------------------------------------------------------------- tree
interface Node { name: string; path: string; dirs: Map<string, Node>; files: FileChange[] }
const collapsed = ref<Set<string>>(new Set())
const tree = computed(() => {
  const root: Node = { name: '', path: '', dirs: new Map(), files: [] }
  for (const f of props.files) {
    const parts = f.path.split('/')
    let node = root
    for (let i = 0; i < parts.length - 1; i++) {
      const key = parts.slice(0, i + 1).join('/')
      let child = node.dirs.get(parts[i])
      if (!child) {
        child = { name: parts[i], path: key, dirs: new Map(), files: [] }
        node.dirs.set(parts[i], child)
      }
      node = child
    }
    node.files.push(f)
  }
  // compact single-child directory chains ("src/components" instead of two levels)
  const compact = (n: Node): Node => {
    for (const [k, d] of n.dirs) n.dirs.set(k, compact(d))
    if (n.name && n.files.length === 0 && n.dirs.size === 1) {
      const only = [...n.dirs.values()][0]
      return { ...only, name: `${n.name}/${only.name}` }
    }
    return n
  }
  return compact(root)
})

interface Line { kind: 'dir'; node: Node; depth: number }
interface FileLine { kind: 'file'; file: FileChange; depth: number }
const lines = computed<(Line | FileLine)[]>(() => {
  if (props.mode === 'path') return props.files.map((f) => ({ kind: 'file', file: f, depth: 0 }))
  const out: (Line | FileLine)[] = []
  const walk = (n: Node, depth: number) => {
    const dirs = [...n.dirs.values()].sort((a, b) => a.name.localeCompare(b.name))
    for (const d of dirs) {
      out.push({ kind: 'dir', node: d, depth })
      if (!collapsed.value.has(d.path)) walk(d, depth + 1)
    }
    for (const f of [...n.files].sort((a, b) => a.path.localeCompare(b.path))) out.push({ kind: 'file', file: f, depth })
  }
  walk(tree.value, 0)
  return out
})
const flat = computed(() => lines.value.filter((l): l is FileLine => l.kind === 'file').map((l) => l.file))

function toggleDir(p: string) {
  const s = new Set(collapsed.value)
  if (s.has(p)) s.delete(p)
  else s.add(p)
  collapsed.value = s
}

defineExpose({ clearSelection: () => (selected.value = new Set()) })
</script>

<template>
  <div class="flex flex-col py-0.5">
    <template v-for="l in lines" :key="l.kind === 'dir' ? 'd:' + l.node.path : 'f:' + l.file.path">
      <div
        v-if="l.kind === 'dir'"
        class="flex h-6 cursor-default items-center gap-1 pr-2 text-[12px] text-muted-foreground hover:bg-[var(--row-hover)]"
        :style="{ paddingLeft: 8 + l.depth * 14 + 'px' }"
        @click="toggleDir(l.node.path)"
      >
        <component :is="collapsed.has(l.node.path) ? ChevronRight : ChevronDown" class="size-3.5 shrink-0" />
        <Folder class="size-3.5 shrink-0 opacity-70" />
        <span class="truncate">{{ l.node.name }}</span>
      </div>
      <div
        v-else
        class="group flex cursor-default items-center gap-1.5 pr-1.5 text-[12.5px]"
        :class="[
          dense ? 'h-6' : 'h-[26px]',
          selected.has(l.file.path) || activePath === l.file.path ? 'bg-[var(--row-selected)]' : 'hover:bg-[var(--row-hover)]',
        ]"
        :style="{ paddingLeft: (mode === 'tree' ? 8 + l.depth * 14 + 18 : 8) + 'px' }"
        :title="`${statusLabel[l.file.status]}: ${l.file.origPath ? l.file.origPath + ' → ' : ''}${l.file.path}`"
        @click="select(l.file, $event)"
        @contextmenu="onCtx(l.file, $event)"
      >
        <span class="mono w-3.5 shrink-0 text-center text-[11px] font-bold" :class="statusClass[l.file.status]">{{ statusLetter[l.file.status] }}</span>
        <span class="flex min-w-0 flex-1 items-baseline gap-1 truncate">
          <template v-if="mode === 'path'">
            <span class="truncate">
              <span v-if="dirname(l.file.path)" class="text-muted-foreground">{{ dirname(l.file.path) }}/</span><span :class="l.file.status === 'deleted' ? 'line-through opacity-70' : ''">{{ basename(l.file.path) }}</span>
            </span>
          </template>
          <span v-else class="truncate" :class="l.file.status === 'deleted' ? 'line-through opacity-70' : ''">{{ basename(l.file.path) }}</span>
          <span v-if="l.file.origPath" class="truncate text-[11px] text-muted-foreground">← {{ basename(l.file.origPath) }}</span>
        </span>
        <span v-if="l.file.binary" class="shrink-0 text-[10.5px] text-muted-foreground">bin</span>
        <span v-else-if="l.file.additions != null && !actionsVisible" class="mono shrink-0 text-[10.5px] group-hover:hidden">
          <span v-if="l.file.additions" class="text-added">+{{ l.file.additions }}</span>
          <span v-if="l.file.deletions" class="ml-1 text-removed">−{{ l.file.deletions }}</span>
        </span>
        <span class="shrink-0 items-center gap-0.5" :class="actionsVisible ? 'flex' : 'hidden group-hover:flex'" @click.stop>
          <slot name="actions" :file="l.file" :selection="selectionFor(l.file)" />
        </span>
      </div>
    </template>
  </div>
</template>
