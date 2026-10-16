<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { api, toAppError } from '@/services/api'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const props = defineProps<{ repo: RepoStore }>()
const ui = useUiStore()
const scope = ref<'global' | 'local'>('global')
const name = ref('')
const email = ref('')
const saving = ref(false)

function load() {
  const id = props.repo.identity
  if (scope.value === 'global') {
    name.value = id?.globalName ?? ''
    email.value = id?.globalEmail ?? ''
  } else {
    name.value = id?.localName ?? ''
    email.value = id?.localEmail ?? ''
  }
}
onMounted(async () => {
  await props.repo.refreshIdentity()
  if (props.repo.identity?.localName || props.repo.identity?.localEmail) scope.value = 'local'
  load()
})

async function save() {
  saving.value = true
  try {
    await api.setIdentity(props.repo.path, scope.value === 'global', name.value, email.value)
    await props.repo.refreshIdentity()
    ui.toast('success', 'Identidade Git salva.')
    ui.closeDialog(true)
  } catch (e) {
    ui.error(toAppError(e))
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <DialogShell title="Identidade Git" description="Nome e e-mail gravados nos commits (user.name / user.email). A configuração local deste repositório tem prioridade sobre a global." @close="ui.closeDialog()">
    <div class="flex gap-1 rounded-lg bg-muted p-1">
      <button v-for="s in (['global', 'local'] as const)" :key="s" class="flex-1 rounded-md px-2 py-1 text-[12.5px]" :class="scope === s ? 'bg-background font-medium shadow-sm' : 'text-muted-foreground'" @click="scope = s; load()">
        {{ s === 'global' ? 'Global (todos os repositórios)' : 'Somente este repositório' }}
      </button>
    </div>
    <Field label="Nome"><Input v-model="name" placeholder="Seu Nome" /></Field>
    <Field label="E-mail" hint="Use o e-mail associado à sua conta no GitHub/GitLab/Bitbucket para que os commits sejam vinculados ao seu perfil.">
      <Input v-model="email" placeholder="voce@exemplo.com" />
    </Field>
    <p class="text-[11.5px] text-muted-foreground">
      Efetivo agora: {{ repo.identity?.effectiveName ?? '—' }} &lt;{{ repo.identity?.effectiveEmail ?? '—' }}&gt;
    </p>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button :disabled="saving" @click="save">Salvar</Button>
    </template>
  </DialogShell>
</template>
