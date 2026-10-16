<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { FolderOpen } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import NativeSelect from '@/components/common/NativeSelect.vue'
import { dirname } from '@/lib/format'
import { refNameError } from '@/lib/validate'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const props = defineProps<{ repo: RepoStore; branch: string | null }>()
const ui = useUiStore()

type Mode = 'existing' | 'new' | 'detached'
const busyBranches = computed(() => new Set(props.repo.worktrees.map((w) => w.branch).filter(Boolean) as string[]))
const available = computed(() => props.repo.localBranches.filter((b) => !busyBranches.value.has(b.short)))
const initialBranch = props.branch && !busyBranches.value.has(props.branch) ? props.branch : null
const mode = ref<Mode>(initialBranch || available.value.length ? 'existing' : 'new')
const branch = ref(initialBranch ?? available.value[0]?.short ?? '')
const newBranch = ref('')
const start = ref(props.branch ?? props.repo.currentBranch ?? 'HEAD')
const openAfter = ref(true)
const repoName = props.repo.summary.name
const parent = dirname(props.repo.path)
const suggested = (b: string) => `${parent}\\${repoName}.worktrees\\${b.replace(/[\\/:*?"<>|]+/g, '-') || 'worktree'}`
const path = ref(suggested(mode.value === 'existing' ? branch.value : 'nova'))
const pathTouched = ref(false)

watch([mode, branch, newBranch], () => {
  if (pathTouched.value) return
  path.value = suggested(mode.value === 'existing' ? branch.value : mode.value === 'new' ? newBranch.value : 'detached')
})

const error = computed(() => {
  if (!path.value.trim()) return 'Escolha a pasta.'
  if (mode.value === 'existing' && !branch.value) return 'Nenhuma branch disponível (todas estão em uso por worktrees).'
  if (mode.value === 'new') {
    if (props.repo.localBranches.some((b) => b.short === newBranch.value.trim())) return 'Já existe uma branch com esse nome.'
    return refNameError(newBranch.value)
  }
  return null
})

async function pick() {
  const dir = await open({ directory: true, title: 'Pasta da nova worktree', defaultPath: parent })
  if (typeof dir === 'string') {
    path.value = dir
    pathTouched.value = true
  }
}

function submit() {
  if (error.value) return
  ui.closeDialog({
    request: {
      path: path.value.trim(),
      branch: mode.value === 'existing' ? branch.value : null,
      newBranch: mode.value === 'new' ? newBranch.value.trim() : null,
      start: mode.value !== 'existing' ? start.value || null : null,
      detach: mode.value === 'detached',
    },
    open: openAfter.value,
  })
}
const modes: { value: Mode; label: string }[] = [
  { value: 'existing', label: 'Branch existente' },
  { value: 'new', label: 'Nova branch' },
  { value: 'detached', label: 'Commit (detached)' },
]
</script>

<template>
  <DialogShell
    title="Criar worktree"
    description="Uma worktree é uma pasta extra ligada a este repositório, com sua própria branch e alterações. Útil para trabalhar em duas branches ao mesmo tempo."
    width="sm:max-w-xl"
    @close="ui.closeDialog()"
  >
    <div class="flex gap-1 rounded-lg bg-muted p-1">
      <button
        v-for="m in modes"
        :key="m.value"
        type="button"
        class="flex-1 rounded-md px-2 py-1 text-[12.5px] transition-colors"
        :class="mode === m.value ? 'bg-background font-medium shadow-sm' : 'text-muted-foreground hover:text-foreground'"
        @click="mode = m.value"
      >
        {{ m.label }}
      </button>
    </div>
    <Field v-if="mode === 'existing'" label="Branch" hint="Branches já usadas por outra worktree não aparecem: o Git não permite a mesma branch em duas worktrees.">
      <NativeSelect v-model="branch" :options="available.map((b) => ({ value: b.short, label: b.short }))" />
    </Field>
    <template v-else>
      <Field v-if="mode === 'new'" label="Nome da nova branch" :error="newBranch ? error : null">
        <Input v-model="newBranch" autofocus placeholder="feature/x" />
      </Field>
      <Field label="Ponto de partida" hint="Branch, tag ou SHA.">
        <Input v-model="start" class="mono" />
      </Field>
    </template>
    <Field label="Pasta de destino" hint="Precisa estar vazia ou não existir.">
      <div class="flex gap-2">
        <Input v-model="path" class="mono text-[12px]" @input="pathTouched = true" />
        <Button variant="outline" size="icon" title="Escolher pasta" @click="pick"><FolderOpen /></Button>
      </div>
    </Field>
    <label class="flex items-center gap-2 text-[12.5px]"><Checkbox v-model="openAfter" /> Abrir a worktree em nova aba</label>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button :disabled="!!error" @click="submit">Criar worktree</Button>
    </template>
  </DialogShell>
</template>
