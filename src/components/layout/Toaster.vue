<script setup lang="ts">
import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from '@lucide/vue'
import { useUiStore, type ToastKind } from '@/stores/ui'

const ui = useUiStore()
const icon: Record<ToastKind, unknown> = { success: CircleCheck, error: CircleAlert, warning: TriangleAlert, info: Info }
const color: Record<ToastKind, string> = { success: 'text-success', error: 'text-destructive', warning: 'text-warning', info: 'text-info' }
</script>

<template>
  <div class="pointer-events-none fixed right-3 bottom-9 z-[100] flex w-[380px] max-w-[calc(100vw-1.5rem)] flex-col gap-2" role="status" aria-live="polite">
    <TransitionGroup
      enter-active-class="transition duration-150 ease-out"
      enter-from-class="translate-y-2 opacity-0"
      leave-active-class="transition duration-100 ease-in"
      leave-to-class="opacity-0"
    >
      <div
        v-for="t in ui.toasts"
        :key="t.id"
        class="pointer-events-auto flex gap-2.5 rounded-lg border bg-popover p-3 shadow-lg ring-1 ring-black/10"
      >
        <component :is="icon[t.kind]" class="mt-0.5 size-4 shrink-0" :class="color[t.kind]" />
        <div class="min-w-0 flex-1">
          <div class="selectable text-[12.5px] leading-snug font-medium">{{ t.title }}</div>
          <div v-if="t.description" class="selectable mt-0.5 text-[12px] leading-snug text-muted-foreground">{{ t.description }}</div>
          <div v-if="t.details || t.action" class="mt-1.5 flex gap-3 text-[12px]">
            <button v-if="t.action" class="font-medium text-primary hover:underline" @click="t.action!.run(); ui.dismiss(t.id)">{{ t.action.label }}</button>
            <button v-if="t.details" class="text-muted-foreground hover:text-foreground hover:underline" @click="ui.detailsText = { title: t.title, text: t.details! }">Detalhes técnicos</button>
          </div>
        </div>
        <button class="h-fit rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground" aria-label="Fechar" @click="ui.dismiss(t.id)"><X class="size-3.5" /></button>
      </div>
    </TransitionGroup>
  </div>
</template>
