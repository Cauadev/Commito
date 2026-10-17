<script setup lang="ts">
import { computed } from 'vue'
import { Bug, Sparkles, TrendingUp } from '@lucide/vue'
import { changelog } from '@/changelog'
import { useAppStore } from '@/stores/app'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'

const ui = useUiStore()
const app = useAppStore()
const current = computed(() => app.info?.version ?? '')
const meta = {
  new: { label: 'Novo', icon: Sparkles, cls: 'bg-primary/15 text-primary' },
  improved: { label: 'Melhoria', icon: TrendingUp, cls: 'bg-success/15 text-success' },
  fixed: { label: 'Correção', icon: Bug, cls: 'bg-warning/15 text-warning' },
}
const fmt = (d: string) => new Date(d + 'T12:00:00').toLocaleDateString('pt-BR', { day: '2-digit', month: 'long', year: 'numeric' })
</script>

<template>
  <DialogShell title="Novidades" :description="`Você está usando a versão ${current}.`" width="sm:max-w-xl" @close="ui.closeDialog()">
    <div class="max-h-[60vh] space-y-5 overflow-auto pr-1">
      <section v-for="v in changelog" :key="v.version">
        <div class="mb-2 flex items-baseline gap-2">
          <h3 class="text-[14px] font-semibold">v{{ v.version }}</h3>
          <span v-if="v.version === current" class="rounded bg-primary/15 px-1.5 text-[10.5px] text-primary">atual</span>
          <span class="ml-auto text-[11.5px] text-muted-foreground">{{ fmt(v.date) }}</span>
        </div>
        <ul class="space-y-1.5">
          <li v-for="(it, i) in v.items" :key="i" class="flex items-start gap-2 text-[12.5px] leading-snug">
            <span class="mt-px flex shrink-0 items-center gap-1 rounded px-1.5 py-px text-[10.5px] font-medium" :class="meta[it.type].cls">
              <component :is="meta[it.type].icon" class="size-3" />{{ meta[it.type].label }}
            </span>
            <span>{{ it.text }}</span>
          </li>
        </ul>
      </section>
    </div>
  </DialogShell>
</template>
