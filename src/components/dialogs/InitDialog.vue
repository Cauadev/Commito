<script setup lang="ts">
import { ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { FolderOpen } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { api, toAppError } from '@/services/api'
import { useAppStore } from '@/stores/app'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const ui = useUiStore()
const app = useAppStore()
const path = ref('')
const branch = ref('main')
const busy = ref(false)

async function pick() {
  const d = await open({ directory: true, title: 'Pasta do novo repositório' })
  if (typeof d === 'string') path.value = d
}
async function submit() {
  if (!path.value.trim()) return
  busy.value = true
  try {
    const r = await api.initRepository(path.value.trim(), branch.value.trim() || null)
    await app.addResolved(r)
    ui.toast('success', `Repositório inicializado em ${r.path}.`)
    ui.closeDialog(true)
  } catch (e) {
    ui.error(toAppError(e))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <DialogShell title="Inicializar repositório" description="Cria um repositório Git vazio (git init) na pasta escolhida. A pasta pode já conter arquivos." @close="ui.closeDialog()">
    <Field label="Pasta">
      <div class="flex gap-2">
        <Input v-model="path" class="mono text-[12px]" placeholder="C:\projetos\meu-app" />
        <Button variant="outline" size="icon" @click="pick"><FolderOpen /></Button>
      </div>
    </Field>
    <Field label="Branch inicial"><Input v-model="branch" /></Field>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button :disabled="!path.trim() || busy" @click="submit">Inicializar</Button>
    </template>
  </DialogShell>
</template>
