<script setup lang="ts">
import { ref } from 'vue'
import { Button } from '@/components/ui/button'
import type { ResetMode } from '@/types/git'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'

const props = defineProps<{ repo: RepoStore; sha: string; branch: string | null; preset?: ResetMode }>()
const ui = useUiStore()
const mode = ref<ResetMode>(props.preset ?? 'mixed')
const options: { value: ResetMode; title: string; text: string; danger?: boolean }[] = [
  { value: 'soft', title: 'Soft', text: 'Move a branch. Alterações dos commits desfeitos ficam preparadas (staged). Nada é perdido.' },
  { value: 'mixed', title: 'Mixed', text: 'Move a branch. Alterações ficam no working directory, fora do stage. Nada é perdido.' },
  { value: 'keep', title: 'Keep', text: 'Move a branch e atualiza arquivos, mas recusa se isso sobrescrever alterações locais.' },
  { value: 'hard', title: 'Hard', text: 'Move a branch e DESCARTA todas as alterações não commitadas. Destrutivo.', danger: true },
]
</script>

<template>
  <DialogShell
    title="Reset"
    :description="`Redefinir '${props.branch ?? 'HEAD'}' para ${props.sha.slice(0, 10)}. Commits posteriores deixam de pertencer à branch (continuam no reflog).`"
    @close="ui.closeDialog()"
  >
    <div class="grid gap-1.5">
      <button
        v-for="o in options"
        :key="o.value"
        type="button"
        class="rounded-lg border px-3 py-2 text-left transition-colors"
        :class="mode === o.value ? (o.danger ? 'border-destructive bg-destructive/10' : 'border-primary bg-primary/10') : 'border-border hover:bg-muted/60'"
        @click="mode = o.value"
      >
        <div class="text-[13px] font-medium" :class="o.danger ? 'text-destructive' : ''">{{ o.title }}</div>
        <div class="text-[12px] text-muted-foreground">{{ o.text }}</div>
      </button>
    </div>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button :variant="mode === 'hard' ? 'destructive' : 'default'" @click="ui.closeDialog({ mode })">Reset {{ mode }}</Button>
    </template>
  </DialogShell>
</template>
