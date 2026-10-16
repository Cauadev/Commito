// Per-repository (per tab) state. Each open repository/worktree gets its own isolated store,
// so indexes, branches and changes of different tabs never mix.
import { defineStore } from 'pinia'
import { computed, reactive, ref, shallowRef } from 'vue'
import { api, newOpId, toAppError } from '@/services/api'
import { useUiStore } from './ui'
import type {
  AppError, CommitDetail, DiffSource, GitRef, GitStatus, GraphPayload, GraphRow, GraphScope, Identity,
  OperationResult, Remote, ResolvedRepo, StashEntry, Submodule, UndoView, Worktree,
} from '@/types/git'

export type Selection =
  | { kind: 'wip' }
  | { kind: 'commits'; hashes: string[]; anchor: string }
  | { kind: 'stash'; hash: string }

export type DiffTarget =
  | { kind: 'working'; path: string; source: DiffSource }
  | { kind: 'commit'; sha: string; base: string | null; path: string; label?: string }
  | { kind: 'blame'; path: string; rev: string | null }

export interface CompareState {
  base: string
  target: string
  files: import('@/types/git').FileChange[]
}

const PAGE = 1500

function createRepoStore(info: ResolvedRepo) {
  return defineStore(`repo:${info.path.toLowerCase()}`, () => {
    const ui = useUiStore()
    const summary = ref<ResolvedRepo>(info)
    const path = info.path

    const status = shallowRef<GitStatus | null>(null)
    const graph = shallowRef<GraphPayload | null>(null)
    const scope = ref<GraphScope>(ui.prefs.graphScope)
    const limit = ref(PAGE)
    const remotes = shallowRef<Remote[]>([])
    const worktrees = shallowRef<Worktree[]>([])
    const submodules = shallowRef<Submodule[]>([])
    const identity = shallowRef<Identity | null>(null)
    const undo = shallowRef<UndoView[]>([])
    const repoAuth = ref<string | null>(null)

    const selection = ref<Selection>({ kind: 'wip' })
    const diffTarget = ref<DiffTarget | null>(null)
    const diffMaximized = ref(false)
    const commitCache = reactive(new Map<string, CommitDetail>())
    const compare = shallowRef<CompareState | null>(null)
    const composer = reactive({ summary: '', description: '', amend: false, signOff: false, noVerify: false })

    const filter = ref('')
    const filterIndex = ref(0)
    const busy = ref<string | null>(null)
    const loading = reactive({ status: false, graph: false, more: false })
    const lastError = ref<AppError | null>(null)
    const lastFetch = ref<number | null>(null)

    // ------------------------------------------------------------- derived
    const refs = computed<GitRef[]>(() => graph.value?.refs ?? [])
    const stashes = computed<StashEntry[]>(() => graph.value?.stashes ?? [])
    const rows = computed<GraphRow[]>(() => graph.value?.rows ?? [])
    const head = computed(() => graph.value?.head ?? null)
    const localBranches = computed(() => refs.value.filter((r) => r.kind === 'local'))
    const remoteBranches = computed(() => refs.value.filter((r) => r.kind === 'remote'))
    const tags = computed(() => refs.value.filter((r) => r.kind === 'tag'))
    const currentBranch = computed(() => status.value?.head.branch ?? head.value?.branch ?? null)
    const changeCount = computed(() => {
      const s = status.value
      if (!s) return 0
      const paths = new Set<string>()
      for (const f of [...s.staged, ...s.unstaged, ...s.conflicted]) paths.add(f.path)
      return paths.size
    })
    const hasChanges = computed(() => changeCount.value > 0)
    const refsByTarget = computed(() => {
      const m = new Map<string, GitRef[]>()
      for (const r of refs.value) {
        const list = m.get(r.target)
        if (list) list.push(r)
        else m.set(r.target, [r])
      }
      return m
    })
    const rowIndex = computed(() => {
      const m = new Map<string, number>()
      rows.value.forEach((r, i) => m.set(r.hash, i))
      return m
    })
    const filterMatches = computed<number[]>(() => {
      const q = filter.value.trim().toLowerCase()
      if (!q) return []
      const out: number[] = []
      rows.value.forEach((r, i) => {
        if (r.kind === 'wip') return
        if (r.subject.toLowerCase().includes(q) || r.authorName.toLowerCase().includes(q) || r.authorEmail.toLowerCase().includes(q) || r.hash.startsWith(q)) out.push(i)
      })
      return out
    })

    // ------------------------------------------------------------- loading
    let statusRunning = false
    let statusPending = false
    async function refreshStatus() {
      if (statusRunning) {
        statusPending = true
        return
      }
      statusRunning = true
      loading.status = true
      try {
        const prevHadChanges = hasChanges.value
        status.value = await api.getStatus(path)
        lastError.value = null
        if (prevHadChanges !== hasChanges.value) void refreshGraph()
      } catch (e) {
        lastError.value = toAppError(e)
      } finally {
        loading.status = false
        statusRunning = false
        if (statusPending) {
          statusPending = false
          void refreshStatus()
        }
      }
    }

    let graphRunning = false
    let graphPending: { reset: boolean } | null = null
    async function refreshGraph(reset = false) {
      if (graphRunning) {
        graphPending = { reset: reset || (graphPending?.reset ?? false) }
        return
      }
      graphRunning = true
      loading.graph = true
      try {
        graph.value = await api.getGraph(path, scope.value, limit.value, hasChanges.value || !!status.value?.operation, reset)
      } catch (e) {
        lastError.value = toAppError(e)
        ui.error(toAppError(e), 'Falha ao carregar o histórico')
      } finally {
        loading.graph = false
        graphRunning = false
        if (graphPending) {
          const p = graphPending
          graphPending = null
          void refreshGraph(p.reset)
        }
      }
    }

    async function loadMore() {
      if (!graph.value?.hasMore || loading.more || graphRunning) return
      loading.more = true
      limit.value += PAGE
      try {
        await refreshGraph()
      } finally {
        loading.more = false
      }
    }

    /** Loads pages until `hash` is part of the graph (used by search). */
    async function ensureLoaded(hash: string, maxRows = 60000): Promise<boolean> {
      while (!rowIndex.value.has(hash) && graph.value?.hasMore && limit.value < maxRows) {
        limit.value += PAGE * 2
        await refreshGraph()
      }
      return rowIndex.value.has(hash)
    }

    async function setScope(s: GraphScope) {
      scope.value = s
      ui.prefs.graphScope = s
      limit.value = PAGE
      await refreshGraph(true)
    }

    async function refreshRemotes() {
      remotes.value = await api.getRemotes(path).catch(() => remotes.value)
    }
    async function refreshWorktrees() {
      worktrees.value = await api.getWorktrees(path).catch(() => worktrees.value)
    }
    async function refreshSubmodules() {
      submodules.value = await api.getSubmodules(path).catch(() => [])
    }
    async function refreshIdentity() {
      identity.value = await api.getIdentity(path).catch(() => null)
    }
    async function refreshUndo() {
      undo.value = await api.listUndo(path).catch(() => [])
    }
    async function refreshAuth() {
      repoAuth.value = await api.getRepoAuth(path).catch(() => null)
    }

    async function loadAll() {
      await refreshStatus()
      await Promise.all([refreshGraph(), refreshRemotes(), refreshWorktrees(), refreshSubmodules(), refreshIdentity(), refreshUndo(), refreshAuth()])
    }

    async function applyChanged(changed: string[]) {
      const tasks: Promise<unknown>[] = []
      if (changed.includes('status')) tasks.push(refreshStatus())
      if (changed.some((c) => c === 'refs' || c === 'graph' || c === 'stashes')) tasks.push(refreshGraph())
      if (changed.includes('worktrees')) tasks.push(refreshWorktrees())
      if (changed.includes('remotes')) tasks.push(refreshRemotes())
      tasks.push(refreshUndo())
      await Promise.all(tasks)
      // Commit details may change (amend, rebase): drop stale caches lazily.
      if (changed.includes('graph')) {
        for (const k of commitCache.keys()) if (!rowIndex.value.has(k)) commitCache.delete(k)
      }
    }

    /** Called by the file watcher (debounced in Rust). */
    let fsTimer: ReturnType<typeof setTimeout> | undefined
    let fsGit = false
    function onFsChange(kind: 'git' | 'worktree') {
      fsGit ||= kind === 'git'
      clearTimeout(fsTimer)
      fsTimer = setTimeout(() => {
        const git = fsGit
        fsGit = false
        if (busy.value) return // the running operation refreshes on completion
        void refreshStatus()
        if (git) {
          void refreshGraph()
          void refreshWorktrees()
          void refreshUndo()
        }
      }, 250)
    }

    // ------------------------------------------------------------- operations
    interface RunOpts {
      network?: boolean
      quiet?: boolean
      cancellable?: boolean
      onError?: (e: AppError) => boolean | void
    }

    async function run(label: string, fn: (opId: string) => Promise<OperationResult>, opts: RunOpts = {}): Promise<OperationResult | null> {
      const opId = newOpId()
      busy.value = label
      if (opts.network) ui.startOp(opId, label, path, opts.cancellable ?? true)
      try {
        const res = await fn(opId)
        if (res.conflicts) {
          ui.toast('warning', res.message, 'Os arquivos em conflito estão no painel de alterações.', { details: res.details ?? null, timeout: 12000 })
          selection.value = { kind: 'wip' }
        } else if (!opts.quiet) {
          ui.toast('success', res.message)
        }
        if (opts.network) lastFetch.value = Date.now()
        busy.value = null
        await applyChanged(res.changed.length ? res.changed : ['status', 'graph'])
        return res
      } catch (e) {
        const err = toAppError(e)
        busy.value = null
        if (!(opts.onError && opts.onError(err))) {
          if (err.code !== 'cancelled') ui.error(err)
          else ui.toast('info', 'Operação cancelada.')
        }
        void refreshStatus()
        if (err.code === 'conflict') void refreshGraph()
        return null
      } finally {
        busy.value = null
        if (opts.network) ui.endOp(opId)
      }
    }

    // ------------------------------------------------------------- selection
    async function commitDetail(hash: string): Promise<CommitDetail | null> {
      const cached = commitCache.get(hash)
      if (cached) return cached
      try {
        const d = await api.getCommit(path, hash)
        commitCache.set(hash, d)
        return d
      } catch (e) {
        ui.error(toAppError(e))
        return null
      }
    }

    function selectWip() {
      selection.value = { kind: 'wip' }
    }
    function selectCommit(hash: string, mode: 'replace' | 'toggle' | 'range' = 'replace') {
      const row = rows.value[rowIndex.value.get(hash) ?? -1]
      if (row?.kind === 'wip') return selectWip()
      if (row?.kind === 'stash') {
        selection.value = { kind: 'stash', hash }
        return
      }
      const cur = selection.value
      if (mode === 'toggle' && cur.kind === 'commits') {
        const set = new Set(cur.hashes)
        if (set.has(hash)) set.delete(hash)
        else set.add(hash)
        const hashes = [...set]
        selection.value = hashes.length ? { kind: 'commits', hashes, anchor: hash } : { kind: 'wip' }
        return
      }
      if (mode === 'range' && cur.kind === 'commits') {
        const a = rowIndex.value.get(cur.anchor) ?? 0
        const b = rowIndex.value.get(hash) ?? 0
        const [lo, hi] = a < b ? [a, b] : [b, a]
        const hashes = rows.value.slice(lo, hi + 1).filter((r) => r.kind === 'commit').map((r) => r.hash)
        selection.value = { kind: 'commits', hashes, anchor: cur.anchor }
        return
      }
      selection.value = { kind: 'commits', hashes: [hash], anchor: hash }
    }

    function openDiff(t: DiffTarget) {
      diffTarget.value = t
    }
    function closeDiff() {
      diffTarget.value = null
      diffMaximized.value = false
    }

    return {
      summary, path, status, graph, scope, limit, remotes, worktrees, submodules, identity, undo, repoAuth,
      selection, diffTarget, diffMaximized, commitCache, compare, composer, filter, filterIndex, busy, loading,
      lastError, lastFetch,
      refs, stashes, rows, head, localBranches, remoteBranches, tags, currentBranch, changeCount, hasChanges,
      refsByTarget, rowIndex, filterMatches,
      refreshStatus, refreshGraph, loadMore, ensureLoaded, setScope, refreshRemotes, refreshWorktrees,
      refreshSubmodules, refreshIdentity, refreshUndo, refreshAuth, loadAll, applyChanged, onFsChange, run,
      commitDetail, selectWip, selectCommit, openDiff, closeDiff,
    }
  })
}

export type RepoStore = ReturnType<ReturnType<typeof createRepoStore>>

const instances = new Map<string, RepoStore>()

export function repoKey(path: string) {
  return path.replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase()
}

export function getRepoStore(info: ResolvedRepo): RepoStore {
  const key = repoKey(info.path)
  let s = instances.get(key)
  if (!s) {
    s = createRepoStore(info)()
    instances.set(key, s)
  }
  return s
}

export function findRepoStore(path: string): RepoStore | undefined {
  return instances.get(repoKey(path))
}

export function disposeRepoStore(path: string) {
  const key = repoKey(path)
  const s = instances.get(key)
  if (s) {
    s.$dispose()
    instances.delete(key)
  }
}
