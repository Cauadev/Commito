<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Copy, Heart, LoaderCircle, RefreshCw } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { copyText } from '@/features/actions'
import { api, toAppError } from '@/services/api'
import { useUiStore } from '@/stores/ui'
import type { AppError } from '@/types/git'
import DialogShell from './DialogShell.vue'

const ui = useUiStore()
const amounts = [5, 10, 25, 50]
const value = ref<number | null>(10)
const custom = ref('')
const loading = ref(false)
const error = ref<AppError | null>(null)
const pix = ref<{ encodedImage: string; payload: string } | null>(null)
const available = ref(true)

async function load() {
  loading.value = true
  error.value = null
  pix.value = null
  try {
    pix.value = await api.donationPix(value.value)
  } catch (e) {
    error.value = toAppError(e)
  } finally {
    loading.value = false
  }
}
function pick(v: number | null) {
  value.value = v
  custom.value = ''
  void load()
}
function applyCustom() {
  const n = Number(custom.value.replace(',', '.'))
  if (!Number.isFinite(n) || n < 1) return
  value.value = Math.round(n * 100) / 100
  void load()
}
onMounted(async () => {
  available.value = await api.donationAvailable().catch(() => false)
  if (available.value) await load()
})
const brl = (n: number) => n.toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' })
</script>

<template>
  <DialogShell title="Apoie o Commito" description="O Commito é gratuito. Se ele ajuda no seu dia a dia, considere uma doação via PIX — qualquer valor ajuda a manter o projeto." width="sm:max-w-md" @close="ui.closeDialog()">
    <template v-if="!available">
      <div class="flex flex-col items-center gap-2 py-6 text-center text-[12.5px] text-muted-foreground">
        <Heart class="size-8 text-pink-400" />
        Doações ainda não foram configuradas nesta versão.
      </div>
    </template>
    <template v-else>
      <div class="flex flex-wrap gap-1.5">
        <button
          v-for="a in amounts"
          :key="a"
          class="rounded-md border px-3 py-1 text-[12.5px] transition-colors"
          :class="value === a && !custom ? 'border-primary bg-primary/15 text-primary' : 'hover:bg-muted'"
          @click="pick(a)"
        >{{ brl(a) }}</button>
        <button class="rounded-md border px-3 py-1 text-[12.5px]" :class="value === null ? 'border-primary bg-primary/15 text-primary' : 'hover:bg-muted'" @click="pick(null)">Valor livre</button>
        <form class="flex items-center gap-1" @submit.prevent="applyCustom">
          <input v-model="custom" inputmode="decimal" class="h-7 w-20 rounded-md border border-input bg-transparent px-2 text-[12.5px] outline-none focus:border-ring" placeholder="Outro" />
        </form>
      </div>

      <div class="flex min-h-64 flex-col items-center justify-center gap-3 rounded-xl border bg-white/95 p-4">
        <LoaderCircle v-if="loading" class="size-6 animate-spin text-slate-500" />
        <template v-else-if="pix">
          <img :src="`data:image/png;base64,${pix.encodedImage}`" alt="QR code PIX para doação" class="size-56 [image-rendering:pixelated]" />
          <span class="text-[12.5px] font-medium text-slate-700">{{ value ? brl(value) : 'Você escolhe o valor no app do banco' }}</span>
        </template>
        <div v-else-if="error" class="flex flex-col items-center gap-2 text-center text-[12.5px] text-slate-700">
          <span>{{ error.message }}</span>
          <Button size="xs" variant="outline" class="text-slate-800" @click="load"><RefreshCw /> Tentar novamente</Button>
        </div>
      </div>

      <div v-if="pix" class="grid gap-1.5">
        <span class="text-[12px] text-muted-foreground">PIX copia e cola</span>
        <div class="flex gap-2">
          <input :value="pix.payload" readonly class="mono h-8 min-w-0 flex-1 rounded-md border border-input bg-transparent px-2 text-[11px] text-muted-foreground" @focus="(e) => (e.target as HTMLInputElement).select()" />
          <Button size="sm" @click="copyText(pix.payload, 'Código PIX copiado.')"><Copy /> Copiar</Button>
        </div>
      </div>
      <p class="text-center text-[11.5px] text-muted-foreground">Obrigado pelo apoio! 💜</p>
    </template>
  </DialogShell>
</template>
