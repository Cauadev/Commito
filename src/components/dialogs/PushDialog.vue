<script setup lang="ts">
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import NativeSelect from '@/components/common/NativeSelect.vue'
import { refNameError } from '@/lib/validate'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const props = defineProps<{ repo: RepoStore; branch: string; force: boolean; noUpstream: boolean }>()
const ui = useUiStore()
const ref0 = props.repo.localBranches.find((b) => b.short === props.branch)
const up = ref0?.upstream ?? null
const remote = ref(up ? up.split('/')[0] : (props.repo.remotes.find((r) => r.name === 'origin')?.name ?? props.repo.remotes[0]?.name ?? ''))
const remoteBranch = ref(up ? up.slice(up.indexOf('/') + 1) : props.branch)
const setUpstream = ref(props.noUpstream)
const force = ref(props.force)
const remoteOptions = computed(() => props.repo.remotes.map((r) => ({ value: r.name, label: `${r.name} — ${r.pushUrl ?? r.fetchUrl ?? ''}` })))
const error = computed(() => refNameError(remoteBranch.value) ?? (remote.value ? null : 'Escolha um remote.'))
</script>

<template>
  <DialogShell
    title="Push"
    :description="props.noUpstream ? `A branch '${props.branch}' ainda não tem upstream. Escolha para onde enviá-la.` : `Enviar '${props.branch}'.`"
    @close="ui.closeDialog()"
  >
    <Field label="Remote">
      <NativeSelect v-model="remote" :options="remoteOptions" />
    </Field>
    <Field label="Branch no remote" :error="error">
      <Input v-model="remoteBranch" />
    </Field>
    <label class="flex items-center gap-2 text-[12.5px]">
      <Checkbox v-model="setUpstream" /> Definir como upstream (-u) para pull/push futuros
    </label>
    <label class="flex items-center gap-2 text-[12.5px]">
      <Checkbox v-model="force" /> <span :class="force ? 'text-destructive' : ''">Force push com --force-with-lease</span>
    </label>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button :disabled="!!error" :variant="force ? 'destructive' : 'default'" @click="ui.closeDialog({ remote, remoteBranch: remoteBranch.trim(), setUpstream, force })">
        {{ force ? 'Force push' : 'Push' }}
      </Button>
    </template>
  </DialogShell>
</template>
