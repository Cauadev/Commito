<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  CircleCheck, CircleUser, CircleX, CloudDownload, ExternalLink, FolderGit2, FolderOpen, FolderPlus, RefreshCw,
  Settings, TriangleAlert, X,
} from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { api, toAppError } from '@/services/api'
import { cloneRepo, initRepo, openRepoFolder } from '@/features/repos'
import { relativeTime } from '@/lib/format'
import { useAppStore } from '@/stores/app'
import { useUiStore } from '@/stores/ui'
import AccountsDialog from '@/components/dialogs/AccountsDialog.vue'
import SettingsDialog from '@/components/dialogs/SettingsDialog.vue'
import AppLogo from './AppLogo.vue'
import FooterActions from './FooterActions.vue'

const app = useAppStore()
const ui = useUiStore()
const info = computed(() => app.info)
const filter = ref('')
const recent = computed(() => app.recent.filter((r) => !filter.value || (r.name + r.path).toLowerCase().includes(filter.value.toLowerCase())))
const rechecking = ref(false)

async function recheck() {
  rechecking.value = true
  try {
    await app.reloadInfo()
  } finally {
    rechecking.value = false
  }
}
async function removeRecent(path: string) {
  await api.removeRecent(path).catch((e) => ui.error(toAppError(e)))
  await app.refreshSettings()
}
function openGitDownload() {
  void api.openUrl('https://git-scm.com/download/win')
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
  <div class="min-h-0 flex-1 overflow-auto">
    <div class="mx-auto flex max-w-4xl flex-col gap-8 px-8 py-10">
      <header class="flex items-center gap-4">
        <AppLogo class="size-12" />
        <div>
          <h1 class="text-[22px] font-semibold tracking-tight">Commito</h1>
          <p class="text-[13px] text-muted-foreground">Cliente Git desktop — histórico em grafo, worktrees e GitHub/GitLab/Bitbucket.</p>
        </div>
        <div class="ml-auto flex gap-1">
          <Button variant="ghost" size="sm" @click="ui.openDialog(AccountsDialog, {})"><CircleUser /> Contas</Button>
          <Button variant="ghost" size="sm" @click="ui.openDialog(SettingsDialog, {})"><Settings /> Configurações</Button>
        </div>
      </header>

      <!-- Git environment -->
      <section v-if="info && (!info.gitFound || !info.gitVersionOk)" class="rounded-xl border border-destructive/40 bg-destructive/10 p-4">
        <div class="flex items-start gap-3">
          <CircleX class="mt-0.5 size-5 shrink-0 text-destructive" />
          <div class="flex-1">
            <h2 class="font-semibold">{{ info.gitFound ? 'Versão do Git muito antiga' : 'Git não encontrado' }}</h2>
            <p class="mt-1 text-[12.5px] text-muted-foreground">
              O Commito usa o Git instalado no Windows para todas as operações (inclui o Git Credential Manager para HTTPS).
              {{ info.gitFound ? `Encontrado: ${info.gitVersion}. É necessário Git 2.36 ou superior.` : 'Instale o Git for Windows e clique em “Verificar novamente”. Se ele estiver instalado num local diferente, informe o caminho do git.exe nas Configurações.' }}
            </p>
            <div class="mt-3 flex gap-2">
              <Button size="sm" @click="openGitDownload"><ExternalLink /> Baixar Git for Windows</Button>
              <Button size="sm" variant="outline" :disabled="rechecking" @click="recheck"><RefreshCw :class="rechecking ? 'animate-spin' : ''" /> Verificar novamente</Button>
              <Button size="sm" variant="ghost" @click="ui.openDialog(SettingsDialog, {})">Informar caminho…</Button>
            </div>
          </div>
        </div>
      </section>

      <section class="grid grid-cols-3 gap-3">
        <button class="group flex flex-col items-start gap-2 rounded-xl border bg-panel p-4 text-left transition-colors hover:border-primary/60 hover:bg-panel-2 disabled:opacity-50" :disabled="!info?.gitFound" @click="openRepoFolder()">
          <FolderOpen class="size-6 text-primary" />
          <span class="font-semibold">Abrir repositório</span>
          <span class="text-[12px] text-muted-foreground">Uma pasta que já contém um repositório Git.</span>
        </button>
        <button class="group flex flex-col items-start gap-2 rounded-xl border bg-panel p-4 text-left transition-colors hover:border-primary/60 hover:bg-panel-2 disabled:opacity-50" :disabled="!info?.gitFound" @click="cloneRepo()">
          <CloudDownload class="size-6 text-primary" />
          <span class="font-semibold">Clonar</span>
          <span class="text-[12px] text-muted-foreground">Por URL HTTPS/SSH ou a partir de uma conta conectada.</span>
        </button>
        <button class="group flex flex-col items-start gap-2 rounded-xl border bg-panel p-4 text-left transition-colors hover:border-primary/60 hover:bg-panel-2 disabled:opacity-50" :disabled="!info?.gitFound" @click="initRepo()">
          <FolderPlus class="size-6 text-primary" />
          <span class="font-semibold">Inicializar</span>
          <span class="text-[12px] text-muted-foreground">Criar um novo repositório (git init) em uma pasta.</span>
        </button>
      </section>

      <section>
        <div class="mb-2 flex items-center gap-3">
          <h2 class="section-label">Recentes</h2>
          <input v-if="app.recent.length > 6" v-model="filter" class="ml-auto h-7 w-56 rounded-md border border-input bg-background px-2 text-[12.5px] outline-none focus:border-ring" placeholder="Filtrar…" />
        </div>
        <div v-if="recent.length" class="overflow-hidden rounded-xl border">
          <div
            v-for="r in recent"
            :key="r.path"
            class="group flex cursor-default items-center gap-3 border-b px-4 py-2 last:border-b-0 hover:bg-[var(--row-hover)]"
            @click="app.openRepo(r.path)"
          >
            <FolderGit2 class="size-4 shrink-0 text-muted-foreground" />
            <div class="min-w-0 flex-1">
              <div class="truncate text-[13px] font-medium">{{ r.name }}</div>
              <div class="truncate text-[11.5px] text-muted-foreground">{{ r.path }}</div>
            </div>
            <span class="shrink-0 text-[11.5px] text-muted-foreground">{{ relativeTime(r.lastOpened) }}</span>
            <button class="rounded p-1 text-muted-foreground opacity-0 group-hover:opacity-100 hover:bg-muted" title="Remover dos recentes" @click.stop="removeRecent(r.path)"><X class="size-3.5" /></button>
          </div>
        </div>
        <p v-else class="rounded-xl border border-dashed p-6 text-center text-[12.5px] text-muted-foreground">Nenhum repositório recente.</p>
      </section>

      <section v-if="info" class="grid grid-cols-2 gap-x-6 gap-y-1.5 rounded-xl border bg-panel p-4 text-[12px]">
        <h2 class="section-label col-span-2 mb-1">Ambiente</h2>
        <span class="flex items-center gap-1.5">
          <component :is="info.gitFound ? CircleCheck : CircleX" class="size-3.5" :class="info.gitFound ? 'text-success' : 'text-destructive'" />
          {{ info.gitVersion ?? 'Git não encontrado' }}
        </span>
        <span class="truncate text-muted-foreground" :title="info.gitPath ?? ''">{{ info.gitPath }}</span>
        <span class="flex items-center gap-1.5">
          <component :is="info.gcmVersion ? CircleCheck : TriangleAlert" class="size-3.5" :class="info.gcmVersion ? 'text-success' : 'text-warning'" />
          Git Credential Manager {{ info.gcmVersion ?? 'não detectado' }}
        </span>
        <span class="truncate text-muted-foreground">credential.helper = {{ info.credentialHelper ?? '(nenhum)' }}</span>
        <span class="flex items-center gap-1.5">
          <component :is="info.lfsVersion ? CircleCheck : TriangleAlert" class="size-3.5" :class="info.lfsVersion ? 'text-success' : 'text-muted-foreground'" />
          {{ info.lfsVersion ? info.lfsVersion.split(' ')[0] : 'Git LFS não instalado' }}
        </span>
        <span class="text-muted-foreground">Commito {{ info.version }}</span>
      </section>
    </div>
  </div>
  <div class="flex h-7 shrink-0 items-center gap-3 border-t bg-chrome px-3 text-[11.5px] text-muted-foreground">
    <span>Commito {{ info?.version }}</span>
    <span class="ml-auto shrink-0 text-faint">{{ info?.gitVersion?.replace('git version ', 'git ') }}</span>
    <FooterActions />
  </div>
  </div>
</template>
