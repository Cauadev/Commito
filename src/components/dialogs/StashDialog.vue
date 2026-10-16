<script setup lang="ts">
import { ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const props = defineProps<{ repo: RepoStore; paths: string[] }>()
const ui = useUiStore()
const message = ref('')
const includeUntracked = ref(true)
const keepIndex = ref(false)
const hasUntracked = props.repo.status?.unstaged.some((f) => f.status === 'untracked') ?? false
</script>

<template>
  <DialogShell
    title="Guardar alterações (stash)"
    :description="props.paths.length ? `${props.paths.length} arquivo(s) selecionado(s) serão guardados.` : 'Todas as alterações locais serão guardadas e o working directory ficará limpo.'"
    @close="ui.closeDialog()"
  >
    <form @submit.prevent="ui.closeDialog({ message, includeUntracked, keepIndex })">
      <Field label="Mensagem (opcional)">
        <Input v-model="message" autofocus placeholder="WIP: ajuste do formulário" />
      </Field>
    </form>
    <label class="flex items-center gap-2 text-[12.5px]" :class="hasUntracked ? '' : 'opacity-60'">
      <Checkbox v-model="includeUntracked" /> Incluir arquivos não rastreados
    </label>
    <label class="flex items-center gap-2 text-[12.5px]">
      <Checkbox v-model="keepIndex" /> Manter alterações preparadas no index (--keep-index)
    </label>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button @click="ui.closeDialog({ message, includeUntracked, keepIndex })">Guardar no stash</Button>
    </template>
  </DialogShell>
</template>
