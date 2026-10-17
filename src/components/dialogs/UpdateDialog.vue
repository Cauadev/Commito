<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import { Download, LoaderCircle, RotateCw } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { api, events, toAppError } from '@/services/api'
import { useAppStore } from '@/stores/app'
import { useUiStore } from '@/stores/ui'
import type { AppError } from '@/types/git'
import DialogShell from './DialogShell.vue'

const ui = useUiStore()
const app = useAppStore()
const installing = ref(false)
const progress = ref<{ downloaded: number; total?: number | null } | null>(null)
const error = ref<AppError | null>(null)
const pct = computed(() => (progress.value?.total ? Math.min(100, Math.round((progress.value.downloaded / progress.value.total) * 100)) : null))
const mb = (n: number) => (n / 1024 / 1024).toFixed(1)

let unlisten: (() => void) | undefined
void events.onUpdateProgress((p) => {
  progress.value = p
}).then((u) => (unlisten = u))
onUnmounted(() => unlisten?.())

async function install() {
  installing.value = true
  error.value = null
  try {
    // Reopen the same tabs after the restart.
    await api.saveSession(app.tabs.map((t) => t.path), app.activePath).catch(() => {})
    await api.updateInstall()
  } catch (e) {
    error.value = toAppError(e)
    installing.value = false
  }
}
</script>

<template>
  <DialogShell
    :title="app.update ? `Atualização disponível: v${app.update.version}` : 'Atualizações'"
    :description="app.update ? `Você está na v${app.update.currentVersion}. A atualização é baixada, verificada pela assinatura digital e instalada; o Commito reinicia em seguida com as mesmas abas.` : 'Nenhuma atualização pendente.'"
    width="sm:max-w-lg"
    @close="!installing && ui.closeDialog()"
  >
    <pre v-if="app.update?.notes" class="selectable max-h-64 overflow-auto rounded-lg border bg-panel p-3 font-sans text-[12.5px] leading-relaxed whitespace-pre-wrap">{{ app.update.notes }}</pre>
    <div v-if="installing" class="grid gap-1.5">
      <div class="flex items-center gap-2 text-[12.5px]">
        <LoaderCircle class="size-4 animate-spin text-primary" />
        {{ progress ? (pct != null ? `Baixando… ${pct}%` : `Baixando… ${mb(progress.downloaded)} MB`) : 'Iniciando download…' }}
      </div>
      <div class="h-1.5 overflow-hidden rounded-full bg-muted"><div class="h-full bg-orange-500 transition-all" :style="{ width: (pct ?? 5) + '%' }" /></div>
      <p class="text-[11.5px] text-muted-foreground">O instalador roda em modo silencioso e o app será reaberto automaticamente.</p>
    </div>
    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/10 p-2 text-[12px]">
      <div class="font-medium text-destructive">{{ error.message }}</div>
      <div v-if="error.details" class="mt-0.5 text-muted-foreground">{{ error.details }}</div>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="installing" @click="ui.closeDialog()">Depois</Button>
      <Button v-if="!app.update" :disabled="app.updateChecking" @click="app.checkForUpdate(true)"><RotateCw :class="app.updateChecking ? 'animate-spin' : ''" /> Verificar</Button>
      <Button v-else class="bg-orange-500 text-white hover:bg-orange-600" :disabled="installing" @click="install"><Download /> Instalar e reiniciar</Button>
    </template>
  </DialogShell>
</template>
