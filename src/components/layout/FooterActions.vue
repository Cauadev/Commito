<script setup lang="ts">
// App-level footer buttons: pending update (orange), release notes and donation.
import { ArrowUpCircle, Heart, ScrollText } from '@lucide/vue'
import { useAppStore } from '@/stores/app'
import { useUiStore } from '@/stores/ui'
import ChangelogDialog from '@/components/dialogs/ChangelogDialog.vue'
import DonationDialog from '@/components/dialogs/DonationDialog.vue'
import UpdateDialog from '@/components/dialogs/UpdateDialog.vue'

const app = useAppStore()
const ui = useUiStore()
</script>

<template>
  <div class="flex shrink-0 items-center gap-1">
    <button
      v-if="app.update"
      class="flex h-5 items-center gap-1 rounded bg-orange-500 px-2 text-[11px] font-semibold text-white shadow-sm hover:bg-orange-600"
      :title="`Atualização pendente: v${app.update.version}. Clique para instalar.`"
      @click="ui.openDialog(UpdateDialog, {})"
    >
      <ArrowUpCircle class="size-3.5" /> Atualizar para v{{ app.update.version }}
    </button>
    <button class="rounded p-1 hover:bg-muted hover:text-foreground" title="Novidades (changelog)" aria-label="Novidades" @click="ui.openDialog(ChangelogDialog, {})">
      <ScrollText class="size-3.5" />
    </button>
    <button class="rounded p-1 hover:bg-muted hover:text-pink-400" title="Apoie o projeto (PIX)" aria-label="Doar" @click="ui.openDialog(DonationDialog, {})">
      <Heart class="size-3.5" />
    </button>
  </div>
</template>
