<script setup lang="ts">
import { ChevronDown, ChevronRight } from '@lucide/vue'

const props = defineProps<{ title: string; count?: number | null; open: boolean; icon?: unknown }>()
const emit = defineEmits<{ toggle: []; ctx: [] }>()
</script>

<template>
  <section class="border-b border-border/60">
    <div class="group flex h-8 cursor-default items-center gap-1.5 px-2 hover:bg-[var(--row-hover)]" @click="emit('toggle')" @contextmenu="emit('ctx')">
      <component :is="props.open ? ChevronDown : ChevronRight" class="size-3.5 shrink-0 text-muted-foreground" />
      <component :is="props.icon" v-if="props.icon" class="size-3.5 shrink-0 text-muted-foreground" />
      <span class="section-label">{{ props.title }}</span>
      <span class="ml-auto flex items-center gap-0.5" @click.stop>
        <slot name="actions" />
      </span>
      <span v-if="props.count != null" class="min-w-5 text-right text-[11.5px] text-muted-foreground tabular-nums">{{ props.count }}</span>
    </div>
    <div v-if="props.open" class="pb-1">
      <slot />
    </div>
  </section>
</template>
