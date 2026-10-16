<script setup lang="ts">
// Draws the lanes crossing one row. Coordinates are row-local, so consecutive rows join
// seamlessly no matter which rows are currently rendered by the virtual list.
import { computed } from 'vue'
import type { GraphRow } from '@/types/git'
import { initials, laneColor } from '@/lib/format'

const props = defineProps<{
  row: GraphRow
  width: number
  height: number
  laneWidth: number
  hasRefs: boolean
  selected: boolean
  isHead: boolean
}>()

const x = (lane: number) => props.laneWidth / 2 + 6 + lane * props.laneWidth

const paths = computed(() => {
  const h = props.height
  const mid = h / 2
  return props.row.edges.map(([kind, from, to, color, dashed]) => {
    const x1 = x(from)
    const x2 = x(to)
    let d: string
    if (kind === 0) d = `M${x1} 0 V${h}`
    else if (kind === 1) d = x1 === x2 ? `M${x1} 0 V${mid}` : `M${x1} 0 C${x1} ${mid * 0.9} ${x2} ${mid * 0.35} ${x2} ${mid}`
    else d = x1 === x2 ? `M${x1} ${mid} V${h}` : `M${x1} ${mid} C${x1} ${mid + mid * 0.65} ${x2} ${mid + mid * 0.1} ${x2} ${h}`
    return { d, color: laneColor(color), dashed: dashed === 1 }
  })
})

const cx = computed(() => x(props.row.lane))
const color = computed(() => laneColor(props.row.color))
const isMerge = computed(() => props.row.kind === 'commit' && props.row.parents.length > 1)
const nodeR = computed(() => (props.row.kind !== 'commit' ? 7 : isMerge.value ? 4.5 : 8.5))
const label = computed(() => (props.row.kind === 'commit' ? initials(props.row.authorName) : ''))
</script>

<template>
  <svg :width="width" :height="height" class="block overflow-visible" aria-hidden="true">
    <line v-if="hasRefs" x1="0" :y1="height / 2" :x2="cx" :y2="height / 2" :stroke="color" stroke-width="1.5" stroke-opacity="0.55" />
    <path
      v-for="(p, i) in paths"
      :key="i"
      :d="p.d"
      :stroke="p.color"
      stroke-width="2"
      fill="none"
      stroke-linecap="round"
      :stroke-dasharray="p.dashed ? '3 3' : undefined"
    />
    <!-- WIP node -->
    <g v-if="row.kind === 'wip'">
      <circle :cx="cx" :cy="height / 2" :r="nodeR" fill="var(--background)" :stroke="color" stroke-width="2" stroke-dasharray="3 2.4" />
    </g>
    <!-- Stash node -->
    <g v-else-if="row.kind === 'stash'">
      <rect :x="cx - 6.5" :y="height / 2 - 6.5" width="13" height="13" rx="3" fill="var(--background)" :stroke="color" stroke-width="1.8" stroke-dasharray="3 2" />
      <path :d="`M${cx - 3} ${height / 2 - 1} h6 M${cx - 3} ${height / 2 + 2} h6`" :stroke="color" stroke-width="1.4" />
    </g>
    <!-- Merge commit: small solid dot -->
    <circle v-else-if="isMerge" :cx="cx" :cy="height / 2" :r="nodeR" :fill="color" :stroke="selected ? 'var(--foreground)' : 'none'" stroke-width="1.5" />
    <!-- Regular commit: avatar-like node with author initials -->
    <g v-else>
      <circle v-if="isHead" :cx="cx" :cy="height / 2" :r="nodeR + 3" fill="none" :stroke="color" stroke-width="1.5" stroke-opacity="0.6" />
      <circle :cx="cx" :cy="height / 2" :r="nodeR" :fill="color" :stroke="selected ? 'var(--foreground)' : 'var(--background)'" stroke-width="1.5" />
      <text
        :x="cx"
        :y="height / 2 + 0.5"
        text-anchor="middle"
        dominant-baseline="central"
        font-size="7.5"
        font-weight="700"
        fill="rgb(10 14 22 / 0.85)"
        style="font-family: var(--font-sans); letter-spacing: -0.2px"
      >{{ label }}</text>
    </g>
  </svg>
</template>
