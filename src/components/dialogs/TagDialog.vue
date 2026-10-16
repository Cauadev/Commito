<script setup lang="ts">
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { refNameError } from '@/lib/validate'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const props = defineProps<{ repo: RepoStore; target: string; label?: string }>()
const ui = useUiStore()
const name = ref('')
const annotated = ref(true)
const message = ref('')
const push = ref(false)
const exists = computed(() => props.repo.tags.some((t) => t.short === name.value.trim()))
const error = computed(() => (exists.value ? 'Já existe uma tag com esse nome.' : refNameError(name.value)))

function submit() {
  if (error.value) return
  if (annotated.value && !message.value.trim()) return
  ui.closeDialog({ name: name.value.trim(), message: annotated.value ? message.value.trim() : null, push: push.value })
}
</script>

<template>
  <DialogShell title="Criar tag" :description="`Tag no commit ${props.label ?? props.target.slice(0, 10)}.`" @close="ui.closeDialog()">
    <Field label="Nome" :error="name ? error : null">
      <Input v-model="name" autofocus placeholder="v1.2.0" />
    </Field>
    <label class="flex items-center gap-2 text-[12.5px]">
      <Checkbox v-model="annotated" /> Tag anotada (com mensagem, autor e data)
    </label>
    <Field v-if="annotated" label="Mensagem">
      <Textarea v-model="message" rows="3" placeholder="Release 1.2.0" />
    </Field>
    <label v-if="props.repo.remotes.length" class="flex items-center gap-2 text-[12.5px]">
      <Checkbox v-model="push" /> Enviar a tag para o remote após criar
    </label>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button :disabled="!!error || (annotated && !message.trim())" @click="submit">Criar tag</Button>
    </template>
  </DialogShell>
</template>
