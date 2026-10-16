<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ArrowDown, ArrowUp, LoaderCircle } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { api, toAppError } from '@/services/api'
import { shortSha } from '@/lib/format'
import type { TodoItem } from '@/types/git'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'

const props = defineProps<{ repo: RepoStore; base: string }>()
const ui = useUiStore()
interface Item { hash: string; subject: string; author: string; action: TodoItem['action'] }
const items = ref<Item[]>([])
const loading = ref(true)
const error = ref<string | null>(null)

onMounted(async () => {
  try {
    const list = await api.rebasePlan(props.repo.path, props.base)
    items.value = list.map((c) => ({ hash: c.hash, subject: c.subject, author: c.authorName, action: 'pick' }))
  } catch (e) {
    error.value = toAppError(e).message
  } finally {
    loading.value = false
  }
})

const actions: { value: TodoItem['action']; label: string; cls: string }[] = [
  { value: 'pick', label: 'pick', cls: 'text-foreground' },
  { value: 'squash', label: 'squash', cls: 'text-info' },
  { value: 'fixup', label: 'fixup', cls: 'text-warning' },
  { value: 'drop', label: 'drop', cls: 'text-destructive' },
]
function move(i: number, d: number) {
  const j = i + d
  if (j < 0 || j >= items.value.length) return
  const a = items.value
  ;[a[i], a[j]] = [a[j], a[i]]
}
const invalid = computed(() => {
  const kept = items.value.filter((i) => i.action !== 'drop')
  if (!kept.length) return 'Ao menos um commit precisa ser mantido.'
  if (kept[0].action === 'squash' || kept[0].action === 'fixup') return 'O primeiro commit mantido não pode ser squash/fixup.'
  return null
})
function submit() {
  if (invalid.value) return
  ui.closeDialog({ todo: items.value.map((i) => ({ action: i.action, hash: i.hash })) })
}
</script>

<template>
  <DialogShell
    title="Rebase interativo"
    :description="`Reorganize os commits após ${shortSha(props.base)} na branch atual (mais antigo no topo). Squash/fixup combinam o commit com o anterior; drop remove. O histórico será reescrito.`"
    width="sm:max-w-2xl"
    @close="ui.closeDialog()"
  >
    <div v-if="loading" class="flex items-center gap-2 py-6 text-muted-foreground"><LoaderCircle class="size-4 animate-spin" /> Carregando commits…</div>
    <p v-else-if="error" class="text-destructive">{{ error }}</p>
    <p v-else-if="!items.length" class="text-muted-foreground">Não há commits entre este ponto e HEAD.</p>
    <div v-else class="max-h-[55vh] overflow-auto rounded-lg border">
      <div v-for="(it, i) in items" :key="it.hash" class="flex items-center gap-2 border-b px-2 py-1.5 last:border-b-0" :class="it.action === 'drop' ? 'opacity-50' : ''">
        <select v-model="it.action" class="mono h-7 rounded-md border bg-transparent px-1 text-[12px]" :class="actions.find((a) => a.value === it.action)?.cls">
          <option v-for="a in actions" :key="a.value" :value="a.value" class="bg-popover">{{ a.label }}</option>
        </select>
        <span class="mono text-[11.5px] text-muted-foreground">{{ shortSha(it.hash) }}</span>
        <span class="min-w-0 flex-1 truncate" :class="it.action === 'drop' ? 'line-through' : ''">{{ it.subject }}</span>
        <span class="max-w-28 truncate text-[11.5px] text-muted-foreground">{{ it.author }}</span>
        <Button variant="ghost" size="icon-xs" :disabled="i === 0" @click="move(i, -1)"><ArrowUp /></Button>
        <Button variant="ghost" size="icon-xs" :disabled="i === items.length - 1" @click="move(i, 1)"><ArrowDown /></Button>
      </div>
    </div>
    <p v-if="invalid && items.length" class="text-[12px] text-destructive">{{ invalid }}</p>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button :disabled="!!invalid || loading || !items.length" @click="submit">Iniciar rebase</Button>
    </template>
  </DialogShell>
</template>
