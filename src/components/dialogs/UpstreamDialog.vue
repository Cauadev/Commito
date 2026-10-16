<script setup lang="ts">
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import NativeSelect from '@/components/common/NativeSelect.vue'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const props = defineProps<{ repo: RepoStore; branch: string }>()
const ui = useUiStore()
const current = props.repo.localBranches.find((b) => b.short === props.branch)?.upstream ?? ''
const sameName = props.repo.remoteBranches.find((r) => r.short.endsWith(`/${props.branch}`))?.short
const value = ref(current || sameName || '')
const options = computed(() => [{ value: '', label: '(nenhum — remover upstream)' }, ...props.repo.remoteBranches.map((r) => ({ value: r.short, label: r.short }))])
</script>

<template>
  <DialogShell title="Upstream (tracking)" :description="`Branch remota que '${props.branch}' acompanha para pull, push e ahead/behind.`" @close="ui.closeDialog()">
    <Field label="Upstream" hint="Se a branch remota ainda não existe, use Push com a opção -u.">
      <NativeSelect v-model="value" :options="options" />
    </Field>
    <template #footer>
      <Button variant="ghost" @click="ui.closeDialog()">Cancelar</Button>
      <Button @click="ui.closeDialog({ upstream: value || null })">Salvar</Button>
    </template>
  </DialogShell>
</template>
