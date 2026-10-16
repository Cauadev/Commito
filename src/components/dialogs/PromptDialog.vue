<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const props = defineProps<{ title: string; label: string; initial?: string; confirmLabel?: string; allowEmpty?: boolean; placeholder?: string }>()
const ui = useUiStore()
const value = ref(props.initial ?? '')
const input = ref<InstanceType<typeof Input> | null>(null)

function submit() {
  const v = value.value.trim()
  if (!v && !props.allowEmpty) return
  ui.closeDialog(v)
}
onMounted(() => {
  const el = (input.value as unknown as { $el?: HTMLInputElement })?.$el
  el?.focus?.()
  el?.select?.()
})
</script>

<template>
  <DialogShell :title="props.title" @close="ui.closeDialog()">
    <form class="grid gap-3" @submit.prevent="submit">
      <Field :label="props.label">
        <Input ref="input" v-model="value" :placeholder="props.placeholder" autofocus />
      </Field>
    </form>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button :disabled="!props.allowEmpty && !value.trim()" @click="submit">{{ props.confirmLabel ?? 'OK' }}</Button>
    </template>
  </DialogShell>
</template>
