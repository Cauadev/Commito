<script setup lang="ts">
// Branch / tag badges for one commit. A local branch and its remote counterpart pointing to the
// same commit are merged into a single badge with both icons.
import { computed } from 'vue'
import { Check, Cloud, FolderGit2, Laptop, Tag } from '@lucide/vue'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import type { GitRef } from '@/types/git'
import { laneColor } from '@/lib/format'

const props = defineProps<{ refs: GitRef[]; color: number; headBranch: string | null; max?: number }>()
const emit = defineEmits<{ dblclick: [ref: GitRef]; ctx: [ref: GitRef, e: MouseEvent] }>()

interface Badge {
  key: string
  label: string
  local?: GitRef
  remotes: GitRef[]
  tag?: GitRef
  isHead: boolean
  worktree: boolean
  primary: GitRef
}

const badges = computed<Badge[]>(() => {
  const out: Badge[] = []
  const locals = props.refs.filter((r) => r.kind === 'local')
  const remotes = props.refs.filter((r) => r.kind === 'remote')
  const used = new Set<string>()
  for (const l of locals) {
    const rs = remotes.filter((r) => r.short === l.upstream || r.short.slice((r.remote?.length ?? 0) + 1) === l.short)
    rs.forEach((r) => used.add(r.name))
    out.push({ key: l.name, label: l.short, local: l, remotes: rs, isHead: l.isHead, worktree: !!l.worktreePath && !l.isHead, primary: l })
  }
  for (const r of remotes) {
    if (used.has(r.name)) continue
    out.push({ key: r.name, label: r.short, remotes: [r], isHead: false, worktree: false, primary: r })
  }
  for (const t of props.refs.filter((r) => r.kind === 'tag')) {
    out.push({ key: t.name, label: t.short, tag: t, remotes: [], isHead: false, worktree: false, primary: t })
  }
  return out.sort((a, b) => Number(b.isHead) - Number(a.isHead) || Number(!!b.local) - Number(!!a.local) || Number(!b.tag) - Number(!a.tag))
})

const max = computed(() => props.max ?? 1)
const shown = computed(() => badges.value.slice(0, max.value))
const hidden = computed(() => badges.value.slice(max.value))
const c = computed(() => laneColor(props.color))
</script>

<template>
  <div class="flex min-w-0 items-center gap-1">
    <div
      v-for="b in shown"
      :key="b.key"
      class="flex h-[20px] min-w-0 cursor-default items-center gap-1 rounded-[5px] border px-1.5 text-[11.5px] font-medium"
      :style="{
        borderColor: `color-mix(in oklch, ${c} 55%, transparent)`,
        background: b.isHead ? `color-mix(in oklch, ${c} 32%, var(--background))` : `color-mix(in oklch, ${c} 16%, var(--background))`,
      }"
      :title="b.tag ? `Tag ${b.label}${b.tag.message ? ': ' + b.tag.message : ''}` : b.local ? `Branch local ${b.label}${b.local.upstream ? ' → ' + b.local.upstream : ''}` : `Branch remota ${b.label}`"
      @dblclick.stop="emit('dblclick', b.primary)"
      @contextmenu="emit('ctx', b.primary, $event)"
    >
      <Check v-if="b.isHead" class="size-3 shrink-0" :style="{ color: c }" />
      <Tag v-if="b.tag" class="size-3 shrink-0 opacity-80" />
      <span class="truncate">{{ b.label }}</span>
      <Laptop v-if="b.local" class="size-3 shrink-0 opacity-70" />
      <Cloud v-if="b.remotes.length" class="size-3 shrink-0 opacity-70" />
      <FolderGit2 v-if="b.worktree" class="size-3 shrink-0 text-warning" />
    </div>
    <Tooltip v-if="hidden.length">
      <TooltipTrigger as-child>
        <span class="shrink-0 rounded-[5px] border border-border bg-muted px-1 text-[11px] text-muted-foreground">+{{ hidden.length }}</span>
      </TooltipTrigger>
      <TooltipContent side="right" class="flex flex-col items-start gap-0.5">
        <span v-for="b in hidden" :key="b.key">{{ b.tag ? 'tag: ' : '' }}{{ b.label }}</span>
      </TooltipContent>
    </Tooltip>
  </div>
</template>
