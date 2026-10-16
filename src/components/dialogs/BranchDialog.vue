<script setup lang="ts">
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { refNameError } from '@/lib/validate'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const props = defineProps<{ repo: RepoStore; start: string | null; startLabel?: string }>()
const ui = useUiStore()
const name = ref('')
const checkout = ref(true)
const touched = ref(false)
const exists = computed(() => props.repo.localBranches.some((b) => b.short === name.value.trim()))
const error = computed(() => (exists.value ? 'Já existe uma branch com esse nome.' : refNameError(name.value)))
const startText = computed(() => props.startLabel ?? (props.start ? props.start.slice(0, 10) : (props.repo.currentBranch ?? 'HEAD')))

function submit() {
  touched.value = true
  if (error.value) return
  ui.closeDialog({ name: name.value.trim(), checkout: checkout.value, start: props.start })
}
</script>

<template>
  <DialogShell title="Criar branch" :description="`A nova branch começará em ${startText}.`" @close="ui.closeDialog()">
    <form class="grid gap-3" @submit.prevent="submit">
      <Field label="Nome da branch" :error="touched || name ? error : null" hint="Ex.: feature/login, fix/timeout">
        <Input v-model="name" autofocus placeholder="feature/minha-alteracao" @blur="name = name.trim().replace(/\s+/g, '-')" />
      </Field>
      <label class="flex items-center gap-2 text-[12.5px]">
        <Checkbox v-model="checkout" /> Fazer checkout da nova branch
      </label>
    </form>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button :disabled="!!error" @click="submit">Criar branch</Button>
    </template>
  </DialogShell>
</template>
