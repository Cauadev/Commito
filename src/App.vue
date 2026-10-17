<script setup lang="ts">
import { onMounted } from 'vue'
import { useEventListener } from '@vueuse/core'
import { LoaderCircle } from '@lucide/vue'
import { TooltipProvider } from '@/components/ui/tooltip'
import TabBar from '@/components/layout/TabBar.vue'
import RepoView from '@/components/layout/RepoView.vue'
import HomeView from '@/components/layout/HomeView.vue'
import Toaster from '@/components/layout/Toaster.vue'
import DialogHost from '@/components/layout/DialogHost.vue'
import ConfirmHost from '@/components/dialogs/ConfirmHost.vue'
import CommandPalette from '@/components/layout/CommandPalette.vue'
import { useAppStore } from '@/stores/app'
import { useUiStore } from '@/stores/ui'

const app = useAppStore()
const ui = useUiStore()

onMounted(() => {
  void app.init()
  // Development-only handle used by end-to-end scripts (never present in production builds).
  if (import.meta.env.DEV) (window as unknown as Record<string, unknown>).__fg = { app, ui }
})

// Ctrl + mouse wheel also zooms.
useEventListener(
  window,
  'wheel',
  (e: WheelEvent) => {
    if (!e.ctrlKey) return
    e.preventDefault()
    ui.stepZoom(e.deltaY < 0 ? 1 : -1)
  },
  { passive: false },
)

const inEditable = (e: KeyboardEvent) => {
  const el = e.target as HTMLElement | null
  return !!el?.closest('input, textarea, select, [contenteditable="true"]')
}

useEventListener(window, 'keydown', (e: KeyboardEvent) => {
  const k = e.key.toLowerCase()
  // Zoom: Ctrl + "+" / "=" / numpad +, Ctrl + "-", Ctrl + 0 (works on ABNT2 and US layouts).
  if ((e.ctrlKey || e.metaKey) && !e.altKey) {
    if (k === '+' || k === '=' || e.code === 'NumpadAdd' || e.code === 'Equal') {
      e.preventDefault()
      ui.stepZoom(1)
      return
    }
    if (k === '-' || k === '_' || e.code === 'NumpadSubtract' || e.code === 'Minus') {
      e.preventDefault()
      ui.stepZoom(-1)
      return
    }
    if (k === '0' || e.code === 'Numpad0') {
      e.preventDefault()
      ui.stepZoom(0)
      return
    }
  }
  if ((e.ctrlKey || e.metaKey) && k === 'k') {
    e.preventDefault()
    ui.paletteOpen = !ui.paletteOpen
    return
  }
  if (e.ctrlKey && e.shiftKey && k === 't') {
    e.preventDefault()
    void app.reopenClosed()
    return
  }
  if (e.ctrlKey && k === 'tab') {
    e.preventDefault()
    const tabs = app.tabs
    if (!tabs.length) return
    const i = tabs.findIndex((t) => t.path === app.activePath)
    const next = tabs[(i + (e.shiftKey ? -1 : 1) + tabs.length) % tabs.length]
    app.activate(next.path)
    return
  }
  if (e.ctrlKey && e.altKey && k === 'f') {
    e.preventDefault()
    document.querySelector<HTMLInputElement>('[data-sidebar-filter]')?.focus()
    return
  }
  if (inEditable(e) || ui.dialog || ui.confirmState) return
  const repo = app.activeRepo
  if (e.key === 'F5') {
    e.preventDefault()
    if (repo) void repo.loadAll()
  } else if (e.key === 'Escape' && repo?.diffTarget) {
    repo.closeDiff()
  } else if (e.ctrlKey && k === 'w' && app.activePath) {
    e.preventDefault()
    void app.closeTab(app.activePath)
  }
})
</script>

<template>
  <TooltipProvider :delay-duration="400">
    <div class="flex h-full flex-col bg-background text-foreground">
      <div v-if="!app.ready" class="flex flex-1 items-center justify-center gap-2 text-muted-foreground">
        <LoaderCircle class="size-5 animate-spin" /> Iniciando Commito…
      </div>
      <template v-else>
        <TabBar />
        <RepoView v-if="app.activeRepo" :key="app.activeRepo.path" :repo="app.activeRepo" />
        <HomeView v-else />
      </template>
    </div>
    <Toaster />
    <DialogHost />
    <ConfirmHost />
    <CommandPalette />
  </TooltipProvider>
</template>
