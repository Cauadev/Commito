<script setup lang="ts">
// Neutral provider glyphs (generic shapes + initial; no third-party logos are bundled).
import type { Provider } from '@/types/git'

const props = defineProps<{ provider: Provider }>()
const colors: Record<Provider, string> = { github: '#94a3b8', gitlab: '#fb923c', bitbucket: '#60a5fa' }
const letters: Record<Provider, string> = { github: 'GH', gitlab: 'GL', bitbucket: 'BB' }
</script>

<template>
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <rect x="1" y="1" width="22" height="22" rx="6" :fill="colors[props.provider]" fill-opacity="0.2" :stroke="colors[props.provider]" stroke-width="1.5" />
    <text x="12" y="12.5" text-anchor="middle" dominant-baseline="central" font-size="8.5" font-weight="700" :fill="colors[props.provider]" style="font-family: var(--font-sans)">{{ letters[props.provider] }}</text>
  </svg>
</template>
