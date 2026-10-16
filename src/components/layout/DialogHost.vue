<script setup lang="ts">
import { Button } from '@/components/ui/button'
import DialogShell from '@/components/dialogs/DialogShell.vue'
import { copyText } from '@/features/actions'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()
</script>

<template>
  <component :is="ui.dialog.component" v-if="ui.dialog" v-bind="ui.dialog.props" />
  <DialogShell v-if="ui.detailsText" :title="ui.detailsText.title" width="sm:max-w-2xl" @close="ui.detailsText = null">
    <pre class="selectable mono max-h-[60vh] overflow-auto rounded-md border bg-panel p-3 text-[11.5px] whitespace-pre-wrap">{{ ui.detailsText.text }}</pre>
    <template #footer>
      <Button variant="ghost" @click="copyText(ui.detailsText!.text)">Copiar</Button>
      <Button @click="ui.detailsText = null">Fechar</Button>
    </template>
  </DialogShell>
</template>
