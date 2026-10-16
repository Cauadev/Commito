<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { TriangleAlert } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'

const ui = useUiStore()
const typed = ref('')
const checked = ref(false)
const opts = computed(() => ui.confirmState?.opts ?? null)
watch(opts, () => {
  typed.value = ''
  checked.value = false
})
const canConfirm = computed(() => !opts.value?.typeToConfirm || typed.value.trim() === opts.value.typeToConfirm)
function ok() {
  if (canConfirm.value) ui.resolveConfirm(true, checked.value)
}
</script>

<template>
  <DialogShell v-if="opts" :title="opts.title" width="sm:max-w-md" @close="ui.resolveConfirm(false)">
    <div class="flex gap-3">
      <TriangleAlert v-if="opts.destructive" class="mt-0.5 size-5 shrink-0 text-destructive" />
      <p class="text-[13px] leading-relaxed text-foreground/90">{{ opts.message }}</p>
    </div>
    <pre v-if="opts.details" class="selectable mono max-h-48 overflow-auto rounded-md border bg-panel p-2 text-[11.5px] whitespace-pre-wrap text-muted-foreground">{{ opts.details }}</pre>
    <label v-if="opts.checkbox" class="flex items-center gap-2 text-[12.5px]">
      <Checkbox v-model="checked" />
      {{ opts.checkbox }}
    </label>
    <div v-if="opts.typeToConfirm" class="grid gap-1.5">
      <span class="text-[12px] text-muted-foreground">Digite <b class="mono text-foreground">{{ opts.typeToConfirm }}</b> para confirmar:</span>
      <Input v-model="typed" autofocus @keydown.enter="ok" />
    </div>
    <template #footer>
      <Button variant="ghost" @click="ui.resolveConfirm(false)">{{ opts.cancelLabel ?? 'Cancelar' }}</Button>
      <Button :variant="opts.destructive ? 'destructive' : 'default'" :disabled="!canConfirm" @click="ok">
        {{ opts.confirmLabel ?? 'Confirmar' }}
      </Button>
    </template>
  </DialogShell>
</template>
