<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { FolderOpen, Globe, Link, LoaderCircle, Lock, Search, X } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import NativeSelect from '@/components/common/NativeSelect.vue'
import ProviderIcon from '@/components/common/ProviderIcon.vue'
import { api, newOpId, toAppError } from '@/services/api'
import { relativeTime } from '@/lib/format'
import { useAppStore } from '@/stores/app'
import { useUiStore } from '@/stores/ui'
import type { RemoteRepository, Workspace } from '@/types/git'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const props = defineProps<{ url?: string; accountId?: string | null; name?: string }>()
const ui = useUiStore()
const app = useAppStore()

const mode = ref<'url' | 'account'>(props.accountId ? 'account' : 'url')
const url = ref(props.url ?? '')
const parentDir = ref('')
const dirName = ref(props.name ?? '')
const dirTouched = ref(false)
const branch = ref('')
const depth = ref('')
const submodules = ref(true)
const useToken = ref(false)

// account browser
const accountId = ref(props.accountId ?? app.accounts[0]?.id ?? '')
const account = computed(() => app.accounts.find((a) => a.id === accountId.value) ?? null)
const repos = ref<RemoteRepository[]>([])
const nextPage = ref<number | null>(1)
const loadingRepos = ref(false)
const repoError = ref<string | null>(null)
const query = ref('')
const workspaces = ref<Workspace[]>([])
const workspace = ref('')
const protocol = ref<'https' | 'ssh'>('https')
const picked = ref<RemoteRepository | null>(null)

onMounted(async () => {
  parentDir.value = await api.defaultCloneDir()
  if (mode.value === 'account') void resetRepos()
})

function deriveName(u: string) {
  const clean = u.trim().replace(/\/+$/, '').replace(/\.git$/, '')
  const last = clean.split(/[/:]/).pop() ?? ''
  return last.replace(/[^A-Za-z0-9._-]/g, '-')
}
watch(url, (u) => {
  if (!dirTouched.value) dirName.value = deriveName(u)
})

async function resetRepos() {
  repos.value = []
  nextPage.value = 1
  repoError.value = null
  if (!account.value) return
  if (account.value.provider === 'bitbucket') {
    try {
      workspaces.value = await api.listWorkspaces(account.value.id)
      if (!workspaces.value.find((w) => w.slug === workspace.value)) workspace.value = workspaces.value[0]?.slug ?? ''
    } catch (e) {
      repoError.value = toAppError(e).message
      return
    }
  }
  await loadMore()
}
async function loadMore() {
  if (!account.value || nextPage.value == null || loadingRepos.value) return
  loadingRepos.value = true
  try {
    const page = await api.listRemoteRepos(account.value.id, nextPage.value, query.value || null, workspace.value || null)
    repos.value.push(...page.items)
    nextPage.value = page.nextPage ?? null
  } catch (e) {
    const err = toAppError(e)
    repoError.value = err.message + (err.recovery ? ` ${err.recovery}` : '')
    nextPage.value = null
  } finally {
    loadingRepos.value = false
  }
}
let qTimer: ReturnType<typeof setTimeout> | undefined
watch(query, () => {
  // GitHub filtering is client-side over loaded pages; GitLab/Bitbucket search server-side.
  if (account.value?.provider === 'github') return
  clearTimeout(qTimer)
  qTimer = setTimeout(() => void resetRepos(), 350)
})
watch([accountId, workspace], () => void resetRepos())
watch(mode, (m) => m === 'account' && !repos.value.length && void resetRepos())

const shown = computed(() => {
  if (account.value?.provider !== 'github' || !query.value) return repos.value
  const q = query.value.toLowerCase()
  return repos.value.filter((r) => r.fullName.toLowerCase().includes(q) || (r.description ?? '').toLowerCase().includes(q))
})

function pick(r: RemoteRepository) {
  picked.value = r
  url.value = (protocol.value === 'ssh' ? r.cloneSsh : r.cloneHttps) ?? r.cloneHttps ?? r.cloneSsh ?? ''
  if (!dirTouched.value) dirName.value = r.name.replace(/[^A-Za-z0-9._-]/g, '-')
}
watch(protocol, () => picked.value && pick(picked.value))

const isHttps = computed(() => url.value.trim().startsWith('https://'))
const tokenAllowed = computed(() => mode.value === 'account' && !!account.value && isHttps.value)

async function choose() {
  const d = await open({ directory: true, title: 'Pasta onde o repositório será clonado', defaultPath: parentDir.value || undefined })
  if (typeof d === 'string') parentDir.value = d
}

// ----------------------------------------------------------------- clone
const opId = ref<string | null>(null)
const running = computed(() => (opId.value ? ui.ops[opId.value] : null))
const cloning = ref(false)
const error = ref<string | null>(null)
const errorDetails = ref<string | null>(null)
const canClone = computed(() => !!url.value.trim() && !!parentDir.value.trim() && !!dirName.value.trim() && !cloning.value)

async function clone() {
  if (!canClone.value) return
  error.value = null
  cloning.value = true
  const id = newOpId()
  opId.value = id
  ui.startOp(id, 'Clonando', null, true)
  try {
    const resolved = await api.cloneRepository(
      {
        url: url.value.trim(),
        parentDir: parentDir.value.trim(),
        dirName: dirName.value.trim(),
        branch: branch.value.trim() || null,
        depth: depth.value ? Number(depth.value) : null,
        recurseSubmodules: submodules.value,
      },
      id,
      tokenAllowed.value && useToken.value ? account.value!.id : null,
    )
    await app.addResolved(resolved)
    ui.toast('success', `Repositório clonado em ${resolved.path}.`)
    ui.closeDialog(true)
  } catch (e) {
    const err = toAppError(e)
    error.value = err.code === 'cancelled' ? 'Clone cancelado.' : `${err.message}${err.recovery ? ' ' + err.recovery : ''}`
    errorDetails.value = err.details ?? null
  } finally {
    ui.endOp(id)
    opId.value = null
    cloning.value = false
  }
}
function cancel() {
  if (opId.value) void api.cancelOperation(opId.value)
  else ui.closeDialog()
}
</script>

<template>
  <DialogShell title="Clonar repositório" width="sm:max-w-3xl" @close="cancel">
    <div class="flex gap-1 rounded-lg bg-muted p-1">
      <button class="flex flex-1 items-center justify-center gap-1.5 rounded-md px-2 py-1 text-[12.5px]" :class="mode === 'url' ? 'bg-background font-medium shadow-sm' : 'text-muted-foreground'" @click="mode = 'url'">
        <Link class="size-3.5" /> Por URL
      </button>
      <button class="flex flex-1 items-center justify-center gap-1.5 rounded-md px-2 py-1 text-[12.5px]" :class="mode === 'account' ? 'bg-background font-medium shadow-sm' : 'text-muted-foreground'" @click="mode = 'account'">
        <Globe class="size-3.5" /> De uma conta conectada
      </button>
    </div>

    <template v-if="mode === 'account'">
      <p v-if="!app.accounts.length" class="rounded-lg border border-dashed p-4 text-center text-[12.5px] text-muted-foreground">
        Nenhuma conta conectada. Conecte GitHub, GitLab ou Bitbucket em “Contas”, ou clone por URL.
      </p>
      <template v-else>
        <div class="flex gap-2">
          <NativeSelect v-model="accountId" class="w-64" :options="app.accounts.map((a) => ({ value: a.id, label: `${a.provider} — ${a.label || a.username}` }))" />
          <NativeSelect v-if="account?.provider === 'bitbucket'" v-model="workspace" class="w-48" :options="workspaces.map((w) => ({ value: w.slug, label: w.name }))" />
          <div class="relative flex-1">
            <Search class="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <input v-model="query" class="h-8 w-full rounded-lg border border-input bg-transparent pl-7 text-[12.5px] outline-none focus:border-ring dark:bg-input/30" placeholder="Buscar repositórios…" />
          </div>
        </div>
        <div class="h-64 overflow-auto rounded-lg border" @scroll="(e) => { const el = e.target as HTMLElement; if (el.scrollTop + el.clientHeight > el.scrollHeight - 80) void loadMore() }">
          <p v-if="repoError" class="p-3 text-[12.5px] text-destructive">{{ repoError }}</p>
          <button
            v-for="r in shown"
            :key="r.id + r.fullName"
            class="flex w-full items-center gap-2 border-b px-3 py-1.5 text-left last:border-b-0"
            :class="picked?.fullName === r.fullName ? 'bg-[var(--row-selected)]' : 'hover:bg-[var(--row-hover)]'"
            @click="pick(r)"
            @dblclick="pick(r); clone()"
          >
            <ProviderIcon :provider="r.provider" class="size-4 shrink-0" />
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-1.5 text-[12.5px]">
                <span class="truncate font-medium">{{ r.fullName }}</span>
                <Lock v-if="r.private" class="size-3 shrink-0 text-warning" />
                <span v-if="r.fork" class="text-[10.5px] text-muted-foreground">fork</span>
              </div>
              <div v-if="r.description" class="truncate text-[11.5px] text-muted-foreground">{{ r.description }}</div>
            </div>
            <span v-if="r.updatedAt" class="shrink-0 text-[11px] text-muted-foreground">{{ relativeTime(Date.parse(r.updatedAt) / 1000) }}</span>
          </button>
          <div v-if="loadingRepos" class="flex items-center justify-center gap-2 p-3 text-[12px] text-muted-foreground"><LoaderCircle class="size-3.5 animate-spin" /> Carregando…</div>
          <button v-else-if="nextPage" class="w-full p-2 text-[12px] text-primary hover:underline" @click="loadMore">Carregar mais</button>
          <p v-else-if="!shown.length && !repoError" class="p-4 text-center text-[12.5px] text-muted-foreground">Nenhum repositório encontrado.</p>
        </div>
        <div class="flex items-center gap-3 text-[12.5px]">
          <span class="text-muted-foreground">Protocolo:</span>
          <label class="flex items-center gap-1"><input v-model="protocol" type="radio" value="https" /> HTTPS</label>
          <label class="flex items-center gap-1"><input v-model="protocol" type="radio" value="ssh" /> SSH</label>
        </div>
      </template>
    </template>

    <Field label="URL do repositório" hint="HTTPS (https://github.com/dono/repo.git) ou SSH (git@github.com:dono/repo.git).">
      <Input v-model="url" class="mono text-[12px]" placeholder="https://github.com/dono/repo.git" :disabled="cloning" />
    </Field>
    <div class="grid grid-cols-[1fr_200px] gap-2">
      <Field label="Clonar em">
        <div class="flex gap-2">
          <Input v-model="parentDir" class="mono text-[12px]" :disabled="cloning" />
          <Button variant="outline" size="icon" :disabled="cloning" @click="choose"><FolderOpen /></Button>
        </div>
      </Field>
      <Field label="Nome da pasta"><Input v-model="dirName" :disabled="cloning" @input="dirTouched = true" /></Field>
    </div>
    <div class="grid grid-cols-[1fr_140px] gap-2">
      <Field label="Branch (opcional)"><Input v-model="branch" placeholder="padrão do remote" :disabled="cloning" /></Field>
      <Field label="Profundidade (shallow)"><Input v-model="depth" type="number" min="0" placeholder="completo" :disabled="cloning" /></Field>
    </div>
    <label class="flex items-center gap-2 text-[12.5px]"><Checkbox v-model="submodules" :disabled="cloning" /> Inicializar submódulos</label>
    <label v-if="tokenAllowed" class="flex items-start gap-2 text-[12.5px]">
      <Checkbox v-model="useToken" class="mt-0.5" :disabled="cloning" />
      <span>
        Autenticar o Git (HTTPS) com o token desta conta
        <span class="block text-[11.5px] text-muted-foreground">Sem esta opção, o Git usa o Git Credential Manager. O token precisa ter permissão de leitura/escrita do repositório; ele é lido do cofre do Windows e nunca entra na URL.</span>
      </span>
    </label>

    <div v-if="running" class="rounded-lg border bg-panel p-2.5">
      <div class="flex items-center gap-2 text-[12px]">
        <LoaderCircle class="size-3.5 animate-spin text-primary" />
        <span class="truncate">{{ running.phase || running.line || 'Conectando…' }}</span>
        <span v-if="running.percent != null" class="ml-auto tabular-nums">{{ running.percent }}%</span>
      </div>
      <div class="mt-1.5 h-1.5 overflow-hidden rounded-full bg-muted"><div class="h-full bg-primary transition-all" :style="{ width: (running.percent ?? 3) + '%' }" /></div>
      <p class="mt-1.5 text-[11px] text-muted-foreground">Se o Git Credential Manager pedir login, uma janela do navegador ou do GCM será aberta.</p>
    </div>
    <p v-if="error" class="flex items-start gap-1.5 text-[12.5px] text-destructive">
      <X class="mt-0.5 size-3.5 shrink-0" />
      <span>{{ error }} <button v-if="errorDetails" class="text-muted-foreground underline" @click="ui.detailsText = { title: 'Detalhes do erro', text: errorDetails! }">Detalhes</button></span>
    </p>

    <template #footer>
      <Button variant="ghost" @click="cancel">{{ cloning ? 'Cancelar clone' : 'Fechar' }}</Button>
      <Button :disabled="!canClone" @click="clone"><LoaderCircle v-if="cloning" class="animate-spin" /> Clonar</Button>
    </template>
  </DialogShell>
</template>
