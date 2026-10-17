<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { CircleCheck, CircleX, FolderOpen, LoaderCircle, TriangleAlert } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import NativeSelect from '@/components/common/NativeSelect.vue'
import { api, toAppError } from '@/services/api'
import { useAppStore } from '@/stores/app'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import type { SshDiagnostics } from '@/types/git'
import DialogShell from './DialogShell.vue'
import Field from './Field.vue'
import IdentityDialog from './IdentityDialog.vue'

const props = defineProps<{ repo?: RepoStore | null }>()
const ui = useUiStore()
const app = useAppStore()
type Tab = 'general' | 'git' | 'repo' | 'ssh'
const tab = ref<Tab>(props.repo ? 'repo' : 'general')

const gitPath = ref(app.settings?.gitPath ?? '')
const cloneDir = ref(app.settings?.cloneDir ?? '')
const terminal = ref(app.settings?.terminal ?? 'auto')
const repoAuth = ref(props.repo?.repoAuth ?? '')
const ssh = ref<SshDiagnostics | null>(null)
const sshLoading = ref(false)

onMounted(async () => {
  if (!cloneDir.value) cloneDir.value = await api.defaultCloneDir()
  if (props.repo) await props.repo.refreshIdentity()
})

async function saveGeneral() {
  try {
    await api.setPreferences(cloneDir.value.trim() || null, terminal.value === 'auto' ? null : terminal.value)
    await app.refreshSettings()
    ui.toast('success', 'Preferências salvas.')
  } catch (e) {
    ui.error(toAppError(e))
  }
}
async function pickClone() {
  const d = await open({ directory: true, defaultPath: cloneDir.value || undefined })
  if (typeof d === 'string') cloneDir.value = d
}
async function pickGit() {
  const f = await open({ title: 'Selecionar git.exe', filters: [{ name: 'git', extensions: ['exe'] }] })
  if (typeof f === 'string') gitPath.value = f
}
async function saveGit() {
  try {
    await api.setGitPath(gitPath.value.trim() || null)
    await app.reloadInfo()
    await app.refreshSettings()
    ui.toast('success', app.info?.gitFound ? `Usando ${app.info.gitVersion}.` : 'Git não encontrado.')
  } catch (e) {
    ui.error(toAppError(e))
  }
}
async function saveRepoAuth() {
  if (!props.repo) return
  try {
    await api.setRepoAuth(props.repo.path, repoAuth.value || null)
    await props.repo.refreshAuth()
    ui.toast('success', repoAuth.value ? 'Este repositório usará o token da conta em remotes HTTPS do mesmo host.' : 'Este repositório usará o Git Credential Manager / SSH.')
  } catch (e) {
    ui.error(toAppError(e))
  }
}
async function setPullConfig(strategy: string, global: boolean) {
  if (!props.repo) return
  try {
    await api.setPullStrategy(props.repo.path, strategy, global)
    await props.repo.refreshIdentity()
    ui.toast('success', 'Configuração de pull gravada no Git.')
  } catch (e) {
    ui.error(toAppError(e))
  }
}
async function runSsh() {
  sshLoading.value = true
  try {
    ssh.value = await api.sshDiagnostics()
  } catch (e) {
    ui.error(toAppError(e))
  } finally {
    sshLoading.value = false
  }
}
const pullConfig = computed(() => {
  const id = props.repo?.identity
  if (!id) return '—'
  if (id.pullFf === 'only') return 'somente fast-forward (pull.ff=only)'
  if (id.pullRebase === 'true') return 'rebase (pull.rebase=true)'
  if (id.pullRebase === 'false') return 'merge (pull.rebase=false)'
  return 'não definido (o Commito usa merge)'
})
const tabs: { id: Tab; label: string }[] = [
  { id: 'general', label: 'Geral' },
  { id: 'git', label: 'Git' },
  { id: 'repo', label: 'Repositório' },
  { id: 'ssh', label: 'SSH / HTTPS' },
]
</script>

<template>
  <DialogShell title="Configurações" width="sm:max-w-2xl" @close="ui.closeDialog()">
    <div class="flex gap-1 rounded-lg bg-muted p-1">
      <button
        v-for="t in tabs"
        :key="t.id"
        class="flex-1 rounded-md px-2 py-1 text-[12.5px] disabled:opacity-40"
        :class="tab === t.id ? 'bg-background font-medium shadow-sm' : 'text-muted-foreground'"
        :disabled="t.id === 'repo' && !repo"
        @click="tab = t.id"
      >{{ t.label }}</button>
    </div>

    <div v-if="tab === 'general'" class="grid gap-3">
      <Field label="Tema">
        <NativeSelect v-model="ui.prefs.theme" :options="[{ value: 'dark', label: 'Escuro (padrão)' }, { value: 'light', label: 'Claro' }]" />
      </Field>
      <Field label="Pasta padrão para clones">
        <div class="flex gap-2"><Input v-model="cloneDir" class="mono text-[12px]" /><Button variant="outline" size="icon" @click="pickClone"><FolderOpen /></Button></div>
      </Field>
      <Field label="Terminal externo">
        <NativeSelect v-model="terminal" :options="[
          { value: 'auto', label: 'Automático (Windows Terminal → PowerShell)' },
          { value: 'powershell', label: 'Windows PowerShell' },
          { value: 'cmd', label: 'Prompt de Comando' },
          { value: 'git-bash', label: 'Git Bash' },
        ]" />
      </Field>
      <Field label="Estratégia padrão do botão Pull">
        <NativeSelect v-model="ui.prefs.pullStrategy" :options="[
          { value: 'config', label: 'Respeitar configuração do Git' },
          { value: 'merge', label: 'Merge' },
          { value: 'rebase', label: 'Rebase' },
          { value: 'ff_only', label: 'Somente fast-forward' },
        ]" />
      </Field>
      <div class="flex justify-end"><Button @click="saveGeneral">Salvar</Button></div>
    </div>

    <div v-else-if="tab === 'git'" class="grid gap-3 text-[12.5px]">
      <div class="grid gap-1 rounded-lg border bg-panel p-3">
        <span class="flex items-center gap-1.5">
          <component :is="app.info?.gitFound ? CircleCheck : CircleX" class="size-4" :class="app.info?.gitFound ? 'text-success' : 'text-destructive'" />
          {{ app.info?.gitVersion ?? 'Git não encontrado' }}
          <span v-if="app.info?.gitFound && !app.info.gitVersionOk" class="text-warning">(recomendado 2.36+)</span>
        </span>
        <span class="text-muted-foreground">{{ app.info?.gitPath }}</span>
        <span class="flex items-center gap-1.5">
          <component :is="app.info?.gcmVersion ? CircleCheck : TriangleAlert" class="size-4" :class="app.info?.gcmVersion ? 'text-success' : 'text-warning'" />
          Git Credential Manager: {{ app.info?.gcmVersion ?? 'não detectado — logins HTTPS podem falhar sem um credential helper' }}
        </span>
        <span class="text-muted-foreground">credential.helper: {{ app.info?.credentialHelper ?? '(nenhum)' }}</span>
        <span class="text-muted-foreground">Git LFS: {{ app.info?.lfsVersion ?? 'não instalado' }}</span>
      </div>
      <Field label="Caminho do git.exe (opcional)" hint="Vazio = detectar automaticamente no PATH e nas pastas padrão do Git for Windows.">
        <div class="flex gap-2"><Input v-model="gitPath" class="mono text-[12px]" placeholder="C:\Program Files\Git\cmd\git.exe" /><Button variant="outline" size="icon" @click="pickGit"><FolderOpen /></Button></div>
      </Field>
      <div class="flex justify-end"><Button @click="saveGit">Aplicar</Button></div>
    </div>

    <div v-else-if="tab === 'repo' && repo" class="grid gap-3 text-[12.5px]">
      <div class="rounded-lg border bg-panel p-3">
        <div class="mb-1 font-medium">Identidade dos commits</div>
        <div class="text-muted-foreground">{{ repo.identity?.effectiveName ?? '—' }} &lt;{{ repo.identity?.effectiveEmail ?? '—' }}&gt;</div>
        <Button size="xs" variant="outline" class="mt-2" @click="ui.openDialog(IdentityDialog, { repo })">Editar identidade…</Button>
      </div>
      <div class="rounded-lg border bg-panel p-3">
        <div class="mb-1 font-medium">Pull (configuração do Git)</div>
        <div class="text-muted-foreground">Atual: {{ pullConfig }}</div>
        <div class="mt-2 flex flex-wrap gap-1.5">
          <Button size="xs" variant="outline" @click="setPullConfig('merge', false)">Merge (este repo)</Button>
          <Button size="xs" variant="outline" @click="setPullConfig('rebase', false)">Rebase (este repo)</Button>
          <Button size="xs" variant="outline" @click="setPullConfig('ff_only', false)">Só fast-forward (este repo)</Button>
          <Button size="xs" variant="ghost" @click="setPullConfig('unset', false)">Remover config local</Button>
        </div>
      </div>
      <div class="rounded-lg border bg-panel p-3">
        <div class="mb-1 font-medium">Autenticação Git (HTTPS) deste repositório</div>
        <p class="text-[12px] text-muted-foreground">
          Padrão: Git Credential Manager (HTTPS) e ssh-agent (SSH). Opcionalmente, use o token de uma conta conectada: ele só é enviado a remotes HTTPS do mesmo host da conta.
        </p>
        <div class="mt-2 flex gap-2">
          <NativeSelect v-model="repoAuth" class="flex-1" :options="[{ value: '', label: 'Git Credential Manager / SSH (padrão)' }, ...app.accounts.map((a) => ({ value: a.id, label: `Token: ${a.provider} — ${a.label || a.username}` }))]" />
          <Button @click="saveRepoAuth">Salvar</Button>
        </div>
      </div>
    </div>

    <div v-else-if="tab === 'ssh'" class="grid gap-3 text-[12.5px]">
      <p class="text-muted-foreground">
        HTTPS: o Git usa o Git Credential Manager (login pelo navegador; credenciais no cofre do Windows). SSH: o Git usa suas chaves, o agente <span class="mono">ssh-agent</span>, <span class="mono">~/.ssh/config</span> e <span class="mono">known_hosts</span>. O Commito nunca lê nem pede sua chave privada.
      </p>
      <Button class="w-fit" variant="outline" :disabled="sshLoading" @click="runSsh"><LoaderCircle v-if="sshLoading" class="animate-spin" /> Diagnosticar SSH</Button>
      <div v-if="ssh" class="grid gap-1 rounded-lg border bg-panel p-3">
        <span class="flex items-center gap-1.5"><component :is="ssh.sshFound ? CircleCheck : CircleX" class="size-4" :class="ssh.sshFound ? 'text-success' : 'text-destructive'" /> Cliente OpenSSH {{ ssh.sshFound ? 'encontrado' : 'não encontrado no PATH' }}</span>
        <span class="flex items-center gap-1.5"><component :is="ssh.agentKeys.length ? CircleCheck : TriangleAlert" class="size-4" :class="ssh.agentKeys.length ? 'text-success' : 'text-warning'" />
          {{ ssh.agentKeys.length ? `${ssh.agentKeys.length} chave(s) no agente` : ssh.agentError }}</span>
        <span v-for="k in ssh.agentKeys" :key="k" class="mono truncate pl-6 text-[11px] text-muted-foreground">{{ k }}</span>
        <span class="text-muted-foreground">Chaves públicas em ~/.ssh: {{ ssh.publicKeys.join(', ') || 'nenhuma' }}</span>
        <span class="text-muted-foreground">~/.ssh/config: {{ ssh.configExists ? 'sim' : 'não' }} · known_hosts: {{ ssh.knownHostsExists ? 'sim' : 'não' }}</span>
        <p class="mt-1 text-[11.5px] text-muted-foreground">Para ativar o agente no Windows: serviço “OpenSSH Authentication Agent” em execução e <span class="mono">ssh-add</span> com sua chave. Na primeira conexão a um host, aceite a chave do host pelo terminal (<span class="mono">ssh -T git@github.com</span>).</p>
      </div>
    </div>
  </DialogShell>
</template>
