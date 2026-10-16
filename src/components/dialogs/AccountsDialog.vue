<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { CircleCheck, CircleX, CloudDownload, ExternalLink, KeyRound, LoaderCircle, Plus, RefreshCw, Trash2, TriangleAlert } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import ProviderIcon from '@/components/common/ProviderIcon.vue'
import { api, toAppError } from '@/services/api'
import { cloneRepo } from '@/features/repos'
import { useAppStore } from '@/stores/app'
import { useUiStore } from '@/stores/ui'
import type { AppError, Provider, ProviderAccount } from '@/types/git'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'

const ui = useUiStore()
const app = useAppStore()

const providers: { id: Provider; name: string; tokenUrl: (base?: string) => string; docs: string }[] = [
  { id: 'github', name: 'GitHub', tokenUrl: (b) => `${b ? b.replace(/\/api\/v3$/, '') : 'https://github.com'}/settings/tokens`, docs: 'https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens' },
  { id: 'gitlab', name: 'GitLab', tokenUrl: (b) => `${b || 'https://gitlab.com'}/-/user_settings/personal_access_tokens`, docs: 'https://docs.gitlab.com/user/profile/personal_access_tokens/' },
  { id: 'bitbucket', name: 'Bitbucket Cloud', tokenUrl: () => 'https://id.atlassian.com/manage-profile/security/api-tokens', docs: 'https://support.atlassian.com/bitbucket-cloud/docs/api-tokens/' },
]

const adding = ref(!app.accounts.length)
const provider = ref<Provider>('github')
const token = ref('')
const baseUrl = ref('')
const email = ref('')
const label = ref('')
const busy = ref(false)
const formError = ref<AppError | null>(null)
const warnings = ref<string[]>([])
const help = ref('')
const p = computed(() => providers.find((x) => x.id === provider.value)!)

watch(provider, async (pr) => {
  help.value = await api.tokenHelp(pr).catch(() => '')
  formError.value = null
}, { immediate: true })

async function add() {
  busy.value = true
  formError.value = null
  warnings.value = []
  try {
    const res = await api.addAccount(provider.value, token.value, baseUrl.value.trim() || null, email.value.trim() || null, label.value.trim() || null)
    token.value = ''
    warnings.value = res.warnings
    await app.refreshSettings()
    ui.toast('success', `Conta ${p.value.name} conectada como ${res.account.username}.`, 'O token foi salvo no Gerenciador de Credenciais do Windows.')
    adding.value = false
    for (const w of res.warnings) ui.toast('warning', w)
  } catch (e) {
    formError.value = toAppError(e)
  } finally {
    busy.value = false
  }
}

const status = ref<Record<string, { state: 'ok' | 'error' | 'testing'; message?: string; warnings?: string[] }>>({})
async function test(a: ProviderAccount) {
  status.value[a.id] = { state: 'testing' }
  try {
    const prof = await api.testAccount(a.id)
    status.value[a.id] = { state: 'ok', warnings: prof.warnings }
    await app.refreshSettings()
  } catch (e) {
    const err = toAppError(e)
    status.value[a.id] = { state: 'error', message: err.message + (err.recovery ? ` ${err.recovery}` : '') }
  }
}
async function remove(a: ProviderAccount) {
  const c = await ui.confirm({
    title: `Desconectar ${a.username}`,
    message: 'O token desta conta será removido do cofre do Windows (somente a entrada do ForgeGraph; credenciais de outros apps e do Git Credential Manager não são tocadas). Repositórios que usavam esta conta voltam a usar o GCM.',
    confirmLabel: 'Desconectar',
    destructive: true,
  })
  if (!c.ok) return
  try {
    await api.removeAccount(a.id)
    await app.refreshSettings()
  } catch (e) {
    ui.error(toAppError(e))
  }
}
function browse(a: ProviderAccount) {
  ui.closeDialog()
  void cloneRepo({ accountId: a.id })
}
const open = (u: string) => void api.openUrl(u).catch((e) => ui.error(toAppError(e)))
const providerName = (id: Provider) => providers.find((x) => x.id === id)?.name ?? id
</script>

<template>
  <DialogShell
    title="Contas conectadas"
    description="Contas de API para listar e clonar repositórios (inclusive privados). As operações Git (fetch/pull/push) continuam funcionando sem conta, via Git Credential Manager ou SSH."
    width="sm:max-w-2xl"
    @close="ui.closeDialog()"
  >
    <div v-if="app.accounts.length" class="grid gap-2">
      <div v-for="a in app.accounts" :key="a.id" class="rounded-lg border bg-panel p-3">
        <div class="flex items-center gap-3">
          <img v-if="a.avatarUrl" :src="a.avatarUrl" class="size-8 rounded-full" alt="" referrerpolicy="no-referrer" />
          <ProviderIcon v-else :provider="a.provider" class="size-8" />
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2 text-[13px]">
              <b class="truncate">{{ a.label || a.displayName || a.username }}</b>
              <span class="rounded bg-muted px-1.5 text-[10.5px] text-muted-foreground">{{ providerName(a.provider) }}</span>
            </div>
            <div class="truncate text-[11.5px] text-muted-foreground">
              @{{ a.username }}{{ a.baseUrl ? ` · ${a.baseUrl}` : '' }}{{ a.email ? ` · ${a.email}` : '' }}{{ a.expiresAt ? ` · expira em ${a.expiresAt}` : '' }}
            </div>
            <div v-if="a.scopes?.length" class="truncate text-[11px] text-faint">escopos: {{ a.scopes.join(', ') }}</div>
          </div>
          <Button size="xs" variant="outline" @click="browse(a)"><CloudDownload /> Repositórios</Button>
          <Button size="icon-xs" variant="ghost" title="Testar conexão da API" @click="test(a)"><RefreshCw :class="status[a.id]?.state === 'testing' ? 'animate-spin' : ''" /></Button>
          <Button size="icon-xs" variant="ghost" class="hover:text-destructive" title="Desconectar" @click="remove(a)"><Trash2 /></Button>
        </div>
        <div v-if="status[a.id] && status[a.id].state !== 'testing'" class="mt-2 text-[12px]">
          <span v-if="status[a.id].state === 'ok'" class="flex items-center gap-1 text-success"><CircleCheck class="size-3.5" /> API válida.</span>
          <span v-else class="flex items-start gap-1 text-destructive"><CircleX class="mt-0.5 size-3.5 shrink-0" /> {{ status[a.id].message }}</span>
          <p v-for="w in status[a.id].warnings ?? []" :key="w" class="mt-1 flex items-start gap-1 text-warning"><TriangleAlert class="mt-0.5 size-3.5 shrink-0" />{{ w }}</p>
        </div>
      </div>
      <p class="text-[11.5px] text-muted-foreground">
        Validar a API não garante permissão de push via Git. Para testar credenciais Git de um repositório, use “Testar conexão” no menu do remote.
      </p>
    </div>

    <Button v-if="!adding" variant="outline" class="w-fit" @click="adding = true"><Plus /> Conectar conta</Button>

    <div v-if="adding" class="grid gap-3 rounded-lg border p-3">
      <div class="flex gap-1 rounded-lg bg-muted p-1">
        <button v-for="x in providers" :key="x.id" class="flex flex-1 items-center justify-center gap-1.5 rounded-md px-2 py-1 text-[12.5px]" :class="provider === x.id ? 'bg-background font-medium shadow-sm' : 'text-muted-foreground'" @click="provider = x.id">
          <ProviderIcon :provider="x.id" class="size-4" /> {{ x.name }}
        </button>
      </div>
      <p class="text-[12px] leading-relaxed text-muted-foreground">{{ help }}</p>
      <div class="flex flex-wrap gap-3 text-[12px]">
        <button class="flex items-center gap-1 text-primary hover:underline" @click="open(p.tokenUrl(baseUrl.trim() || undefined))"><KeyRound class="size-3.5" /> Gerar token</button>
        <button class="flex items-center gap-1 text-primary hover:underline" @click="open(p.docs)"><ExternalLink class="size-3.5" /> Documentação</button>
      </div>
      <Field v-if="provider !== 'bitbucket'" :label="provider === 'gitlab' ? 'URL do GitLab (self-managed, opcional)' : 'URL do GitHub Enterprise Server (opcional)'" hint="Deixe vazio para github.com / gitlab.com. Precisa ser HTTPS.">
        <Input v-model="baseUrl" class="mono text-[12px]" :placeholder="provider === 'gitlab' ? 'https://gitlab.minhaempresa.com' : 'https://github.minhaempresa.com'" />
      </Field>
      <Field v-if="provider === 'bitbucket'" label="E-mail da conta Atlassian" hint="Usado com o API token na autenticação da REST API (App Passwords foram descontinuadas).">
        <Input v-model="email" placeholder="voce@empresa.com" />
      </Field>
      <Field :label="provider === 'bitbucket' ? 'API token (com escopos)' : 'Personal Access Token'" hint="Armazenado apenas no Gerenciador de Credenciais do Windows.">
        <Input v-model="token" type="password" autocomplete="off" class="mono" placeholder="••••••••••••" @keydown.enter="add" />
      </Field>
      <Field label="Rótulo (opcional)" hint="Útil quando houver mais de uma conta no mesmo provedor."><Input v-model="label" placeholder="Trabalho" /></Field>
      <div v-if="formError" class="rounded-md border border-destructive/40 bg-destructive/10 p-2 text-[12px]">
        <div class="font-medium text-destructive">{{ formError.message }}</div>
        <div v-if="formError.recovery" class="mt-0.5 text-muted-foreground">{{ formError.recovery }}</div>
      </div>
      <p v-for="w in warnings" :key="w" class="flex items-start gap-1 text-[12px] text-warning"><TriangleAlert class="mt-0.5 size-3.5 shrink-0" />{{ w }}</p>
      <div class="flex justify-end gap-2">
        <Button v-if="app.accounts.length" variant="ghost" @click="adding = false">Cancelar</Button>
        <Button :disabled="busy || !token.trim() || (provider === 'bitbucket' && !email.trim())" @click="add">
          <LoaderCircle v-if="busy" class="animate-spin" /> Validar e conectar
        </Button>
      </div>
    </div>
  </DialogShell>
</template>
