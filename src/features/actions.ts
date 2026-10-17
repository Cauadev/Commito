// User-facing Git flows: confirmations, dialogs, retries. Shared by toolbar, menus,
// sidebar, graph and command palette so behaviour is identical everywhere.
import { api, toAppError } from '@/services/api'
import { useAppStore } from '@/stores/app'
import type { RepoStore } from '@/stores/repo'
import { useUiStore } from '@/stores/ui'
import { shortSha } from '@/lib/format'
import type { AppError, CheckoutTarget, FileChange, GitRef, MergeMode, PullStrategy, ResetMode, StashEntry, Worktree } from '@/types/git'
import PromptDialog from '@/components/dialogs/PromptDialog.vue'
import BranchDialog from '@/components/dialogs/BranchDialog.vue'
import TagDialog from '@/components/dialogs/TagDialog.vue'
import MergeDialog from '@/components/dialogs/MergeDialog.vue'
import ResetDialog from '@/components/dialogs/ResetDialog.vue'
import PushDialog from '@/components/dialogs/PushDialog.vue'
import StashDialog from '@/components/dialogs/StashDialog.vue'
import WorktreeDialog from '@/components/dialogs/WorktreeDialog.vue'
import RemoteDialog from '@/components/dialogs/RemoteDialog.vue'
import UpstreamDialog from '@/components/dialogs/UpstreamDialog.vue'
import RebaseDialog from '@/components/dialogs/RebaseDialog.vue'

export async function copyText(text: string, label = 'Copiado para a área de transferência.') {
  try {
    await navigator.clipboard.writeText(text)
    useUiStore().toast('info', label, null, { timeout: 2000 })
  } catch {
    useUiStore().toast('error', 'Não foi possível copiar.')
  }
}

export function commitWebUrl(webUrl: string, sha: string) {
  if (webUrl.includes('bitbucket.org')) return `${webUrl}/commits/${sha}`
  if (webUrl.includes('gitlab')) return `${webUrl}/-/commit/${sha}`
  return `${webUrl}/commit/${sha}`
}

export function useActions(repo: RepoStore) {
  const ui = useUiStore()
  const app = useAppStore()
  const p = repo.path

  const defaultRemote = () => repo.remotes.find((r) => r.name === 'origin')?.name ?? repo.remotes[0]?.name ?? null

  // ---------------------------------------------------------------- sync
  async function fetchAll(remote: string | null = null, prune = false) {
    if (!repo.remotes.length) await repo.refreshRemotes()
    if (!repo.remotes.length) {
      ui.toast('info', 'Este repositório não tem remotes.', 'Adicione um remote na barra lateral para sincronizar.')
      return
    }
    await repo.run(remote ? `Fetch ${remote}` : 'Fetch', (op) => api.fetch(p, remote, prune, op), { network: true })
  }

  async function pull(strategy?: PullStrategy) {
    const s = strategy ?? ui.prefs.pullStrategy
    await repo.run('Pull', (op) => api.pull(p, s, op), {
      network: true,
      onError: (e) => {
        if (e.code === 'non_fast_forward' && s !== 'rebase' && s !== 'merge') {
          ui.toast('warning', e.message, e.recovery, {
            details: e.details ?? null,
            timeout: 15000,
            action: { label: 'Pull com rebase', run: () => void pull('rebase') },
          })
          return true
        }
        if (e.code === 'no_upstream') {
          ui.toast('warning', e.message, e.recovery, { action: { label: 'Definir upstream…', run: () => void setUpstream(repo.currentBranch!) } })
          return true
        }
        if (e.code === 'dirty_worktree') {
          ui.toast('warning', e.message, 'Faça stash ou commit antes do pull.', {
            details: e.details ?? null,
            action: { label: 'Stash…', run: () => void stashSave() },
          })
          return true
        }
        return false
      },
    })
  }

  async function push(opts: { force?: boolean; branch?: string } = {}) {
    if (!repo.remotes.length) await repo.refreshRemotes()
    const branch = opts.branch ?? repo.currentBranch
    if (!branch) {
      ui.toast('warning', 'HEAD destacado: faça checkout de uma branch para enviar.')
      return
    }
    const ref = repo.localBranches.find((r) => r.short === branch)
    const upstream = ref?.upstream ?? null
    if (!repo.remotes.length) {
      ui.toast('warning', 'Este repositório não tem remotes.', 'Adicione um remote antes do push.')
      return
    }
    let remote: string
    let remoteBranch: string
    let setUpstream = false
    let force = !!opts.force
    if (!upstream || ref?.upstreamGone || force) {
      const res = await ui.openDialog<{ remote: string; remoteBranch: string; setUpstream: boolean; force: boolean }>(PushDialog, {
        repo,
        branch,
        force,
        noUpstream: !upstream,
      })
      if (!res) return
      ;({ remote, remoteBranch, setUpstream, force } = res)
    } else {
      const slash = upstream.indexOf('/')
      remote = upstream.slice(0, slash)
      remoteBranch = upstream.slice(slash + 1)
    }
    if (force) {
      const c = await ui.confirm({
        title: 'Force push (with lease)',
        message: `Isto reescreve '${remote}/${remoteBranch}' com a sua branch local '${branch}'. Commits que só existem no remote serão descartados lá. O --force-with-lease recusa se alguém tiver enviado algo que você ainda não buscou.`,
        confirmLabel: 'Forçar push',
        destructive: true,
        typeToConfirm: remoteBranch,
      })
      if (!c.ok) return
    }
    await repo.run(force ? 'Force push' : 'Push', (op) => api.push(p, { remote, branch, remoteBranch, setUpstream, forceWithLease: force }, op), {
      network: true,
      onError: (e) => {
        if (e.code === 'non_fast_forward') {
          ui.toast('warning', e.message, e.recovery, {
            details: e.details ?? null,
            timeout: 20000,
            action: { label: 'Pull agora', run: () => void pull() },
          })
          return true
        }
        return false
      },
    })
  }

  // ---------------------------------------------------------------- branches
  async function createBranch(start?: string, startLabel?: string) {
    const res = await ui.openDialog<{ name: string; checkout: boolean; start: string | null }>(BranchDialog, { repo, start: start ?? null, startLabel })
    if (!res) return
    await repo.run('Criar branch', () => api.createBranch(p, res.name, res.start, res.checkout))
  }

  async function doCheckout(target: CheckoutTarget, label: string) {
    const attempt = (autoStash: boolean) =>
      repo.run(label, () => api.checkout(p, target, autoStash), {
        onError: (e) => {
          if (e.code === 'dirty_worktree') {
            void (async () => {
              const c = await ui.confirm({
                title: 'Alterações locais em conflito com o checkout',
                message: 'Você tem alterações que seriam sobrescritas. Deseja guardá-las num stash automático, trocar de branch e reaplicá-las em seguida?',
                details: e.details ?? undefined,
                confirmLabel: 'Stash automático e checkout',
              })
              if (c.ok) await attempt(true)
            })()
            return true
          }
          return false
        },
      })
    return attempt(false)
  }

  async function checkoutRef(ref: GitRef) {
    if (ref.kind === 'local') {
      if (ref.isHead) return
      if (ref.worktreePath && !samePath(ref.worktreePath, p)) {
        const c = await ui.confirm({
          title: 'Branch em uso por outra worktree',
          message: `'${ref.short}' está ativa em ${ref.worktreePath}. O Git não permite a mesma branch em duas worktrees. Abrir essa worktree em uma nova aba?`,
          confirmLabel: 'Abrir worktree',
        })
        if (c.ok) await app.openRepo(ref.worktreePath)
        return
      }
      return doCheckout({ kind: 'branch', name: ref.short }, `Checkout ${ref.short}`)
    }
    if (ref.kind === 'remote') {
      const localName = ref.short.slice((ref.remote?.length ?? 0) + 1)
      const tracking = repo.localBranches.find((b) => b.upstream === ref.short)
      if (tracking) return checkoutRef(tracking)
      const existing = repo.localBranches.find((b) => b.short === localName)
      let name = localName
      if (existing) {
        const n = await ui.openDialog<string>(PromptDialog, {
          title: 'Criar branch local',
          label: `Já existe uma branch local '${localName}' que não rastreia '${ref.short}'. Escolha outro nome:`,
          initial: `${localName}-${ref.remote}`,
          confirmLabel: 'Criar e trocar',
        })
        if (!n) return
        name = n
      }
      return doCheckout({ kind: 'remote', remote_branch: ref.short, local_name: name }, `Checkout ${ref.short}`)
    }
    return checkoutCommit(ref.target, ref.short)
  }

  async function checkoutCommit(sha: string, label?: string) {
    const c = await ui.confirm({
      title: 'Checkout em HEAD destacado',
      message: `Você ficará em ${label ?? shortSha(sha)} sem branch (detached HEAD). Novos commits feitos aqui ficam sem branch e podem se perder ao trocar de branch, a menos que você crie uma.`,
      confirmLabel: 'Fazer checkout',
    })
    if (!c.ok) return
    return doCheckout({ kind: 'detached', rev: sha }, `Checkout ${label ?? shortSha(sha)}`)
  }

  async function renameBranch(ref: GitRef) {
    const n = await ui.openDialog<string>(PromptDialog, { title: 'Renomear branch', label: 'Novo nome', initial: ref.short, confirmLabel: 'Renomear' })
    if (!n || n === ref.short) return
    await repo.run('Renomear branch', () => api.renameBranch(p, ref.short, n))
  }

  async function deleteBranch(ref: GitRef) {
    if (ref.isHead) {
      ui.toast('warning', 'Não é possível excluir a branch atual.', 'Faça checkout de outra branch antes.')
      return
    }
    const c = await ui.confirm({
      title: `Excluir branch '${ref.short}'`,
      message: 'A branch local será removida. Commits que existem apenas nela continuarão acessíveis pelo Undo e pelo reflog por um tempo.',
      confirmLabel: 'Excluir',
      destructive: true,
      checkbox: ref.upstream && !ref.upstreamGone ? `Excluir também '${ref.upstream}' no remote` : undefined,
    })
    if (!c.ok) return
    const res = await repo.run('Excluir branch', () => api.deleteBranch(p, ref.short, false), {
      onError: (e) => {
        if (e.code === 'invalid_input' && e.message.includes('commits')) {
          void (async () => {
            const f = await ui.confirm({
              title: 'Branch não mesclada',
              message: `${e.message} Excluir mesmo assim?`,
              confirmLabel: 'Excluir à força',
              destructive: true,
              typeToConfirm: ref.short,
            })
            if (f.ok) {
              await repo.run('Excluir branch', () => api.deleteBranch(p, ref.short, true))
              if (c.checked && ref.upstream) await deleteRemoteBranchByName(ref.upstream)
            }
          })()
          return true
        }
        return false
      },
    })
    if (res && c.checked && ref.upstream) await deleteRemoteBranchByName(ref.upstream)
  }

  async function deleteRemoteBranchByName(full: string) {
    const slash = full.indexOf('/')
    const remote = full.slice(0, slash)
    const branch = full.slice(slash + 1)
    await repo.run(`Excluir ${full}`, (op) => api.deleteRemoteBranch(p, remote, branch, op), { network: true })
  }

  async function deleteRemoteBranch(ref: GitRef) {
    const c = await ui.confirm({
      title: `Excluir '${ref.short}' do remote`,
      message: `A branch será apagada no servidor '${ref.remote}' para todos os colaboradores. Esta ação não pode ser desfeita pelo Commito.`,
      confirmLabel: 'Excluir no remote',
      destructive: true,
      typeToConfirm: ref.short.slice((ref.remote?.length ?? 0) + 1),
    })
    if (!c.ok) return
    await deleteRemoteBranchByName(ref.short)
  }

  async function setUpstream(branch: string) {
    const res = await ui.openDialog<{ upstream: string | null }>(UpstreamDialog, { repo, branch })
    if (!res) return
    await repo.run('Definir upstream', () => api.setUpstream(p, branch, res.upstream))
  }

  async function mergeInto(target: string, label?: string) {
    const cur = repo.currentBranch
    if (!cur) {
      ui.toast('warning', 'HEAD destacado: faça checkout de uma branch antes do merge.')
      return
    }
    const mode = await ui.openDialog<MergeMode>(MergeDialog, { repo, source: label ?? target, target: cur })
    if (!mode) return
    await repo.run(`Merge ${label ?? target}`, () => api.merge(p, target, mode))
  }

  async function rebaseOnto(onto: string, label?: string) {
    const cur = repo.currentBranch
    if (!cur) {
      ui.toast('warning', 'HEAD destacado: faça checkout de uma branch antes do rebase.')
      return
    }
    const c = await ui.confirm({
      title: `Rebase de '${cur}' sobre '${label ?? shortSha(onto)}'`,
      message: `Os commits de '${cur}' serão reaplicados sobre '${label ?? shortSha(onto)}', reescrevendo o histórico local. Se a branch já foi enviada, será necessário force push depois.`,
      confirmLabel: 'Fazer rebase',
    })
    if (!c.ok) return
    await repo.run('Rebase', () => api.rebase(p, onto))
  }

  async function interactiveRebase(base: string) {
    const res = await ui.openDialog<{ todo: import('@/types/git').TodoItem[] }>(RebaseDialog, { repo, base })
    if (!res) return
    await repo.run('Rebase interativo', () => api.rebaseInteractive(p, base, res.todo))
  }

  async function cherryPick(shas: string[]) {
    // Apply in chronological order (oldest first): graph rows are newest first.
    const ordered = [...shas].sort((a, b) => (repo.rowIndex.get(b) ?? 0) - (repo.rowIndex.get(a) ?? 0))
    const cur = repo.currentBranch ?? 'HEAD destacado'
    const list = ordered.map((h) => `• ${shortSha(h)} ${repo.rows[repo.rowIndex.get(h) ?? -1]?.subject ?? ''}`).join('\n')
    const c = await ui.confirm({
      title: `Cherry-pick em '${cur}'`,
      message: `${ordered.length} commit(s) serão aplicados em '${cur}', nesta ordem:`,
      details: list,
      confirmLabel: 'Aplicar',
    })
    if (!c.ok) return
    await repo.run('Cherry-pick', () => api.cherryPick(p, ordered))
  }

  async function revert(sha: string) {
    const row = repo.rows[repo.rowIndex.get(sha) ?? -1]
    const c = await ui.confirm({
      title: 'Reverter commit',
      message: `Será criado um novo commit que desfaz as alterações de ${shortSha(sha)}${row ? ` ("${row.subject}")` : ''}. O histórico existente não é alterado.`,
      confirmLabel: 'Reverter',
    })
    if (!c.ok) return
    await repo.run('Revert', () => api.revert(p, sha))
  }

  async function reset(sha: string, preset?: ResetMode) {
    const cur = repo.currentBranch
    const res = await ui.openDialog<{ mode: ResetMode }>(ResetDialog, { repo, sha, branch: cur, preset })
    if (!res) return
    if (res.mode === 'hard') {
      const c = await ui.confirm({
        title: 'Reset hard',
        message: `Todas as alterações não commitadas (preparadas e não preparadas) serão PERDIDAS e '${cur ?? 'HEAD'}' passará a apontar para ${shortSha(sha)}. Arquivos não rastreados não são afetados.`,
        confirmLabel: 'Descartar e redefinir',
        destructive: true,
        typeToConfirm: 'reset',
      })
      if (!c.ok) return
    }
    await repo.run(`Reset ${res.mode}`, () => api.reset(p, sha, res.mode, res.mode === 'hard'))
  }

  // ---------------------------------------------------------------- tags
  async function createTag(target: string, label?: string) {
    const res = await ui.openDialog<{ name: string; message: string | null; push: boolean }>(TagDialog, { repo, target, label })
    if (!res) return
    const ok = await repo.run('Criar tag', () => api.createTag(p, res.name, target, res.message))
    if (ok && res.push) {
      const remote = defaultRemote()
      if (remote) await repo.run('Enviar tag', (op) => api.pushTag(p, remote, res.name, op), { network: true })
    }
  }

  async function deleteTag(ref: GitRef) {
    const c = await ui.confirm({
      title: `Excluir tag '${ref.short}'`,
      message: 'A tag local será removida.',
      confirmLabel: 'Excluir',
      destructive: true,
      checkbox: repo.remotes.length ? `Excluir também do remote '${defaultRemote()}'` : undefined,
    })
    if (!c.ok) return
    await repo.run('Excluir tag', () => api.deleteTag(p, ref.short))
    if (c.checked) {
      const remote = defaultRemote()
      if (remote) await repo.run('Excluir tag remota', (op) => api.deleteRemoteTag(p, remote, ref.short, op), { network: true })
    }
  }

  async function pushTag(ref: GitRef) {
    const remote = defaultRemote()
    if (!remote) return ui.toast('warning', 'Sem remotes configurados.')
    await repo.run('Enviar tag', (op) => api.pushTag(p, remote, ref.short, op), { network: true })
  }

  async function renameTag(ref: GitRef) {
    const n = await ui.openDialog<string>(PromptDialog, {
      title: 'Renomear tag',
      label: 'Novo nome (a tag é recriada no mesmo objeto; tags já enviadas ao remote não são alteradas)',
      initial: ref.short,
      confirmLabel: 'Renomear',
    })
    if (!n || n === ref.short) return
    await repo.run('Renomear tag', () => api.renameTag(p, ref.short, n))
  }

  // ---------------------------------------------------------------- stash
  async function stashSave(paths: string[] = []) {
    const res = await ui.openDialog<{ message: string; includeUntracked: boolean; keepIndex: boolean }>(StashDialog, { repo, paths })
    if (!res) return
    await repo.run('Stash', () => api.stashPush(p, { message: res.message || null, includeUntracked: res.includeUntracked, keepIndex: res.keepIndex, paths }))
  }

  async function stashQuick() {
    if (!repo.hasChanges) return ui.toast('info', 'Não há alterações para guardar.')
    await repo.run('Stash', () => api.stashPush(p, { includeUntracked: true }))
  }

  async function stashApply(s: StashEntry, pop: boolean) {
    await repo.run(pop ? 'Stash pop' : 'Aplicar stash', () => api.stashApply(p, s.index, s.hash, pop))
  }

  async function stashPopLatest() {
    const s = repo.stashes[0]
    if (!s) return ui.toast('info', 'Não há stashes.')
    await stashApply(s, true)
  }

  async function stashDrop(s: StashEntry) {
    const c = await ui.confirm({ title: 'Descartar stash', message: `Descartar ${s.reference} ("${s.message}")? Pode ser desfeito com Undo logo em seguida.`, confirmLabel: 'Descartar', destructive: true })
    if (!c.ok) return
    await repo.run('Descartar stash', () => api.stashDrop(p, s.index, s.hash))
  }

  async function stashClear() {
    const c = await ui.confirm({
      title: 'Limpar todos os stashes',
      message: `Todos os ${repo.stashes.length} stashes serão removidos. Esta ação não pode ser desfeita pela interface.`,
      confirmLabel: 'Limpar tudo',
      destructive: true,
      typeToConfirm: 'limpar',
    })
    if (!c.ok) return
    await repo.run('Limpar stashes', () => api.stashClear(p))
  }

  async function stashBranch(s: StashEntry) {
    const n = await ui.openDialog<string>(PromptDialog, { title: 'Criar branch a partir do stash', label: 'Nome da nova branch', initial: '', confirmLabel: 'Criar' })
    if (!n) return
    await repo.run('Branch do stash', () => api.stashBranch(p, s.index, s.hash, n))
  }

  // ---------------------------------------------------------------- worktrees
  async function createWorktree(branch?: string) {
    const res = await ui.openDialog<{ request: import('@/types/git').WorktreeRequest; open: boolean }>(WorktreeDialog, { repo, branch: branch ?? null })
    if (!res) return
    const ok = await repo.run('Criar worktree', () => api.worktreeAdd(p, res.request))
    if (ok && res.open) await app.openRepo(res.request.path)
  }

  async function openWorktree(w: Worktree) {
    if (w.prunable) return ui.toast('warning', 'A pasta desta worktree não existe mais.', 'Use "Limpar worktrees obsoletas" (prune).')
    await app.openRepo(w.path)
  }

  async function removeWorktree(w: Worktree) {
    if (w.isMain) return ui.toast('warning', 'A worktree principal não pode ser removida.')
    if (w.locked) return ui.toast('warning', `A worktree está bloqueada${w.lockReason ? `: ${w.lockReason}` : ''}.`, 'Desbloqueie antes de remover.')
    const dirty = w.dirty === true
    const c = await ui.confirm({
      title: 'Remover worktree',
      message: dirty
        ? `A worktree em ${w.path} tem ${w.changedFiles ?? 'algumas'} alteração(ões) não commitada(s). Removê-la apagará a pasta e essas alterações serão PERDIDAS.`
        : `A pasta ${w.path} será removida. A branch '${w.branch ?? '(detached)'}' e seus commits continuam no repositório.`,
      confirmLabel: dirty ? 'Remover e perder alterações' : 'Remover',
      destructive: true,
      typeToConfirm: dirty ? 'remover' : undefined,
    })
    if (!c.ok) return
    const open = app.tabs.find((t) => samePath(t.path, w.path))
    if (open) await app.closeTab(open.path)
    await repo.run('Remover worktree', () => api.worktreeRemove(p, w.path, dirty))
  }

  async function lockWorktree(w: Worktree) {
    if (w.locked) return repo.run('Desbloquear worktree', () => api.worktreeLock(p, w.path, false, null))
    const reason = await ui.openDialog<string>(PromptDialog, { title: 'Bloquear worktree', label: 'Motivo (opcional)', initial: '', allowEmpty: true, confirmLabel: 'Bloquear' })
    if (reason === undefined) return
    await repo.run('Bloquear worktree', () => api.worktreeLock(p, w.path, true, reason || null))
  }

  async function pruneWorktrees() {
    await repo.run('Prune worktrees', () => api.worktreePrune(p))
  }

  // ---------------------------------------------------------------- remotes
  async function addRemote() {
    const res = await ui.openDialog<{ name: string; url: string; pushUrl: string | null }>(RemoteDialog, { repo })
    if (!res) return
    const ok = await repo.run('Adicionar remote', () => api.remoteAdd(p, res.name, res.url))
    if (ok && res.pushUrl) await repo.run('URL de push', () => api.remoteSetUrl(p, res.name, res.pushUrl!, true))
    if (ok) await fetchAll(res.name)
  }

  async function editRemote(name: string) {
    const r = repo.remotes.find((x) => x.name === name)
    if (!r) return
    const res = await ui.openDialog<{ name: string; url: string; pushUrl: string | null }>(RemoteDialog, { repo, remote: r })
    if (!res) return
    if (res.name !== r.name) await repo.run('Renomear remote', () => api.remoteRename(p, r.name, res.name))
    if (res.url !== r.fetchUrl) await repo.run('URL do remote', () => api.remoteSetUrl(p, res.name, res.url, false))
    if (res.pushUrl && res.pushUrl !== r.pushUrl) await repo.run('URL de push', () => api.remoteSetUrl(p, res.name, res.pushUrl!, true))
  }

  async function removeRemote(name: string) {
    const c = await ui.confirm({
      title: `Remover remote '${name}'`,
      message: 'A configuração do remote e suas referências locais (remote-tracking) serão removidas. Nada é apagado no servidor.',
      confirmLabel: 'Remover',
      destructive: true,
    })
    if (!c.ok) return
    await repo.run('Remover remote', () => api.remoteRemove(p, name))
  }

  async function testRemote(name: string) {
    await repo.run(`Testar ${name}`, () => api.testRemote(p, name, null), { network: true })
  }

  async function pruneRemote(name: string) {
    await repo.run(`Prune ${name}`, (op) => api.pruneRemote(p, name, op), { network: true })
  }

  // ---------------------------------------------------------------- working tree
  async function stage(files: FileChange[] | 'all') {
    await repo.run('Preparar', () => api.stage(p, files === 'all' ? [] : files.map((f) => f.path)), { quiet: true })
  }
  async function unstage(files: FileChange[] | 'all') {
    await repo.run('Remover do stage', () => api.unstage(p, files === 'all' ? [] : files.map((f) => f.path)), { quiet: true })
  }

  async function discard(files: FileChange[]) {
    if (!files.length) return
    const untracked = files.filter((f) => f.status === 'untracked')
    const tracked = files.filter((f) => f.status !== 'untracked')
    const parts: string[] = []
    if (tracked.length) parts.push(`${tracked.length} arquivo(s) rastreado(s) voltarão ao conteúdo do index/último commit; as alterações não preparadas serão perdidas`)
    if (untracked.length) parts.push(`${untracked.length} arquivo(s) não rastreado(s) serão movidos para a Lixeira do Windows`)
    const c = await ui.confirm({
      title: files.length === 1 ? `Descartar alterações em ${files[0].path}` : `Descartar ${files.length} arquivos`,
      message: parts.join('; ') + '. Alterações já preparadas (staged) não são afetadas.',
      details: files.length > 1 ? files.map((f) => f.path).join('\n') : undefined,
      confirmLabel: 'Descartar',
      destructive: true,
    })
    if (!c.ok) return
    await repo.run('Descartar', () => api.discard(p, tracked.map((f) => f.path), untracked.map((f) => f.path)))
  }

  async function commit() {
    const c = repo.composer
    const summary = c.summary.trim()
    if (!summary) {
      ui.toast('warning', 'Escreva um resumo para o commit.')
      return false
    }
    const message = c.description.trim() ? `${summary}\n\n${c.description.trim()}` : summary
    if (!c.amend && !(repo.status?.staged.length)) {
      ui.toast('warning', 'Nenhuma alteração preparada.', 'Prepare arquivos (stage) antes de fazer commit.')
      return false
    }
    const res = await repo.run(c.amend ? 'Amend' : 'Commit', () => api.commit(p, { message, amend: c.amend, signOff: c.signOff, noVerify: c.noVerify }))
    if (res?.ok) {
      c.summary = ''
      c.description = ''
      c.amend = false
      repo.selectWip()
      repo.closeDiff()
      return true
    }
    return false
  }

  async function toggleAmend(on: boolean) {
    const c = repo.composer
    c.amend = on
    if (on && !c.summary.trim()) {
      try {
        const msg = await api.lastCommitMessage(p)
        const [first, ...rest] = msg.split('\n')
        c.summary = first
        c.description = rest.join('\n').trim()
      } catch (e) {
        ui.error(toAppError(e))
      }
    }
  }

  // ---------------------------------------------------------------- sequencer & conflicts
  async function sequencer(action: 'continue' | 'abort' | 'skip') {
    if (action === 'abort') {
      const c = await ui.confirm({
        title: 'Abortar operação',
        message: 'O repositório voltará ao estado anterior ao início da operação. Resoluções de conflito feitas até aqui serão descartadas.',
        confirmLabel: 'Abortar',
        destructive: true,
      })
      if (!c.ok) return
    }
    await repo.run(action === 'continue' ? 'Continuar' : action === 'abort' ? 'Abortar' : 'Pular', () => api.sequencer(p, action))
  }

  async function resolve(files: FileChange[], how: 'ours' | 'theirs' | 'mark_resolved') {
    await repo.run('Resolver conflito', () => api.resolveConflicts(p, files.map((f) => f.path), how), { quiet: true })
  }

  // ---------------------------------------------------------------- misc
  async function undo() {
    await repo.refreshUndo()
    const top = repo.undo[0]
    if (!top) return ui.toast('info', 'Nada para desfazer.', 'Apenas operações locais comprovadamente reversíveis feitas nesta sessão podem ser desfeitas.')
    if (!top.available) return ui.toast('warning', `Não é possível desfazer "${top.label}".`, top.reason)
    if (top.warning) {
      const c = await ui.confirm({ title: `Desfazer: ${top.label}`, message: top.warning, confirmLabel: 'Desfazer' })
      if (!c.ok) return
    }
    try {
      const msg = await api.undoLast(p, top.id)
      ui.toast('success', msg)
    } catch (e) {
      ui.error(toAppError(e))
    }
    await repo.applyChanged(['status', 'graph', 'worktrees'])
  }

  function openRemoteWeb(remoteName?: string) {
    const r = repo.remotes.find((x) => x.name === (remoteName ?? defaultRemote()))
    if (!r?.webUrl) return ui.toast('info', 'Não foi possível derivar uma URL web para este remote.')
    void api.openUrl(r.webUrl).catch((e) => ui.error(toAppError(e)))
  }

  function openCommitWeb(sha: string) {
    const r = repo.remotes.find((x) => x.name === defaultRemote())
    if (!r?.webUrl) return ui.toast('info', 'Sem remote com URL web.')
    void api.openUrl(commitWebUrl(r.webUrl, sha)).catch((e) => ui.error(toAppError(e)))
  }

  const reportError = (e: AppError) => ui.error(e)

  return {
    defaultRemote, fetchAll, pull, push, createBranch, checkoutRef, checkoutCommit, renameBranch, deleteBranch,
    deleteRemoteBranch, setUpstream, mergeInto, rebaseOnto, interactiveRebase, cherryPick, revert, reset,
    createTag, deleteTag, pushTag, renameTag, stashSave, stashQuick, stashApply, stashPopLatest, stashDrop,
    stashClear, stashBranch, createWorktree, openWorktree, removeWorktree, lockWorktree, pruneWorktrees,
    addRemote, editRemote, removeRemote, testRemote, pruneRemote, stage, unstage, discard, commit, toggleAmend,
    sequencer, resolve, undo, openRemoteWeb, openCommitWeb, reportError,
  }
}

export function samePath(a: string, b: string) {
  const n = (s: string) => s.replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase()
  return n(a) === n(b)
}

export type Actions = ReturnType<typeof useActions>
