<script setup lang="ts">
import { ref } from 'vue'
import { Button } from '@/components/ui/button'
import type { MergeMode } from '@/types/git'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'

const props = defineProps<{ repo: RepoStore; source: string; target: string }>()
const ui = useUiStore()
const mode = ref<MergeMode>('default')
const options: { value: MergeMode; title: string; text: string }[] = [
  { value: 'default', title: 'Padrão', text: 'Fast-forward quando possível; caso contrário, cria um commit de merge.' },
  { value: 'no_ff', title: 'Sempre criar commit de merge (--no-ff)', text: 'Preserva no grafo que estes commits vieram de outra branch.' },
  { value: 'ff_only', title: 'Somente fast-forward (--ff-only)', text: 'Falha se as branches divergiram; nunca cria commit de merge.' },
  { value: 'squash', title: 'Squash', text: 'Combina as alterações no index sem commitar; você revisa e faz um único commit.' },
]
</script>

<template>
  <DialogShell title="Merge" :description="`Mesclar '${props.source}' em '${props.target}' (branch atual).`" @close="ui.closeDialog()">
    <div class="grid gap-1.5">
      <button
        v-for="o in options"
        :key="o.value"
        type="button"
        class="rounded-lg border px-3 py-2 text-left transition-colors"
        :class="mode === o.value ? 'border-primary bg-primary/10' : 'border-border hover:bg-muted/60'"
        @click="mode = o.value"
      >
        <div class="text-[13px] font-medium">{{ o.title }}</div>
        <div class="text-[12px] text-muted-foreground">{{ o.text }}</div>
      </button>
    </div>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button @click="ui.closeDialog(mode)">Mesclar</Button>
    </template>
  </DialogShell>
</template>
