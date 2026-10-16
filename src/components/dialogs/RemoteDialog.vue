<script setup lang="ts">
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import type { Remote } from '@/types/git'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const props = defineProps<{ repo: RepoStore; remote?: Remote }>()
const ui = useUiStore()
const name = ref(props.remote?.name ?? (props.repo.remotes.length ? '' : 'origin'))
const url = ref(props.remote?.fetchUrl ?? '')
const pushUrl = ref(props.remote?.pushUrl && props.remote.pushUrl !== props.remote.fetchUrl ? props.remote.pushUrl : '')

const error = computed(() => {
  if (!/^[A-Za-z0-9._-]+$/.test(name.value.trim())) return 'Nome inválido.'
  if (!props.remote && props.repo.remotes.some((r) => r.name === name.value.trim())) return 'Já existe um remote com esse nome.'
  const u = url.value.trim()
  if (!u) return 'Informe a URL.'
  if (/^https?:\/\/[^/@\s]+:[^/@\s]+@/.test(u)) return 'Não inclua senha/token na URL. Use GCM, SSH ou uma conta conectada.'
  return null
})
</script>

<template>
  <DialogShell :title="props.remote ? `Editar remote '${props.remote.name}'` : 'Adicionar remote'" @close="ui.closeDialog()">
    <Field label="Nome"><Input v-model="name" autofocus placeholder="origin" /></Field>
    <Field label="URL (fetch)" hint="HTTPS (https://host/owner/repo.git) ou SSH (git@host:owner/repo.git)">
      <Input v-model="url" class="mono text-[12px]" placeholder="git@github.com:usuario/repo.git" />
    </Field>
    <Field label="URL de push (opcional)" hint="Deixe vazio para usar a mesma URL.">
      <Input v-model="pushUrl" class="mono text-[12px]" />
    </Field>
    <p v-if="error && url" class="text-[12px] text-destructive">{{ error }}</p>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button :disabled="!!error" @click="ui.closeDialog({ name: name.trim(), url: url.trim(), pushUrl: pushUrl.trim() || null })">
        {{ props.remote ? 'Salvar' : 'Adicionar e buscar' }}
      </Button>
    </template>
  </DialogShell>
</template>
