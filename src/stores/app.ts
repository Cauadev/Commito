// Application-level state: environment info, settings, repository tabs and session restore.
import { defineStore } from 'pinia'
import { computed, ref, shallowRef } from 'vue'
import { api, events, toAppError, type UpdateInfo } from '@/services/api'
import { disposeRepoStore, findRepoStore, getRepoStore, type RepoStore } from './repo'
import { useUiStore } from './ui'
import type { AppInfo, ProviderAccount, ResolvedRepo, Settings } from '@/types/git'

export const useAppStore = defineStore('app', () => {
  const ui = useUiStore()
  const info = shallowRef<AppInfo | null>(null)
  const settings = ref<Settings | null>(null)
  const tabs = ref<ResolvedRepo[]>([])
  const activePath = ref<string | null>(null)
  const closedTabs = ref<string[]>([])
  const ready = ref(false)
  const online = ref(navigator.onLine)
  /** Repositories being opened (shown as loading tabs). */
  const opening = ref<string[]>([])
  /** Bumped on every navigation the user makes; async opens only steal focus if it is unchanged. */
  let navEpoch = 0
  let restoring = false

  // ---------------------------------------------------------------- auto-update
  const update = ref<UpdateInfo | null>(null)
  const updateConfigured = ref(false)
  const updateChecking = ref(false)
  async function checkForUpdate(manual = false) {
    if (!updateConfigured.value) return null
    updateChecking.value = true
    try {
      update.value = await api.updateCheck()
      if (manual && !update.value) ui.toast('info', 'Você já está na versão mais recente.')
      return update.value
    } catch (e) {
      if (manual) ui.error(toAppError(e))
      return null
    } finally {
      updateChecking.value = false
    }
  }

  const activeTab = computed(() => tabs.value.find((t) => t.path === activePath.value) ?? null)
  const activeRepo = computed<RepoStore | null>(() => (activeTab.value ? getRepoStore(activeTab.value) : null))
  const accounts = computed<ProviderAccount[]>(() => settings.value?.accounts ?? [])
  const recent = computed(() => settings.value?.recentRepos ?? [])

  async function refreshSettings() {
    settings.value = await api.getSettings()
  }

  async function init() {
    window.addEventListener('online', () => (online.value = true))
    window.addEventListener('offline', () => (online.value = false))
    await events.onProgress((p) => ui.progress(p))
    await events.onRepoChanged((p) => findRepoStore(p.repo)?.onFsChange(p.kind))
    try {
      info.value = await api.appInfo()
      await refreshSettings()
      ui.loadPrefs(settings.value?.ui)
    } catch (e) {
      ui.error(toAppError(e))
    }
    ready.value = true
    updateConfigured.value = await api.updateConfigured().catch(() => false)
    if (updateConfigured.value) {
      setTimeout(() => void checkForUpdate(), 4000)
      setInterval(() => void checkForUpdate(), 6 * 60 * 60 * 1000)
    }
    if (info.value?.gitFound && settings.value) {
      const toOpen = settings.value.openTabs ?? []
      const active = settings.value.activeTab
      const epoch = navEpoch
      restoring = true
      // Open all saved tabs in parallel, keeping their order; a slow repository never blocks the others.
      const results = await Promise.all(toOpen.map((p) => resolveQuiet(p)))
      for (const r of results) if (r && !tabs.value.some((t) => samePathKey(t.path, r.path))) tabs.value.push(r)
      restoring = false
      if (navEpoch === epoch) {
        const want = tabs.value.find((t) => active && samePathKey(t.path, active)) ?? tabs.value[0]
        if (want) activate(want.path, false)
      }
      persistSession()
    }
  }

  async function reloadInfo() {
    info.value = await api.appInfo()
  }

  function samePathKey(a: string, b: string) {
    const n = (s: string) => s.replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase()
    return n(a) === n(b)
  }

  async function resolveQuiet(path: string) {
    opening.value.push(path)
    try {
      return await api.openRepository(path)
    } catch {
      return null
    } finally {
      opening.value = opening.value.filter((p) => p !== path)
    }
  }

  function persistSession() {
    if (restoring) return
    api.saveSession(tabs.value.map((t) => t.path), activePath.value).catch(() => {})
  }

  function activate(path: string, userAction = true) {
    if (userAction) navEpoch++
    activePath.value = path
    persistSession()
    const t = tabs.value.find((x) => x.path === path)
    if (t) {
      const store = getRepoStore(t)
      if (!store.status) void store.loadAll()
    }
  }

  async function addResolved(resolved: ResolvedRepo, activateTab = true, epoch?: number) {
    if (!tabs.value.some((t) => t.path.toLowerCase() === resolved.path.toLowerCase())) {
      tabs.value.push(resolved)
    }
    const existing = tabs.value.find((t) => t.path.toLowerCase() === resolved.path.toLowerCase())!
    // Only take focus if the user did not navigate elsewhere while this repository was opening.
    if (activateTab && (epoch === undefined || epoch === navEpoch)) activate(existing.path, false)
    else persistSession()
    void refreshSettings()
    return existing
  }

  async function openRepo(path: string, opts: { activate?: boolean; quiet?: boolean } = {}) {
    const already = tabs.value.find((t) => samePathKey(t.path, path))
    if (already) {
      if (opts.activate ?? true) activate(already.path)
      return already
    }
    navEpoch++
    const epoch = navEpoch
    opening.value.push(path)
    try {
      const resolved = await api.openRepository(path)
      return await addResolved(resolved, opts.activate ?? true, epoch)
    } catch (e) {
      if (!opts.quiet) ui.error(toAppError(e), 'Não foi possível abrir o repositório')
      return null
    } finally {
      opening.value = opening.value.filter((p) => p !== path)
    }
  }

  async function closeTab(path: string) {
    const idx = tabs.value.findIndex((t) => t.path === path)
    if (idx < 0) return
    tabs.value.splice(idx, 1)
    closedTabs.value.push(path)
    await api.closeRepository(path).catch(() => {})
    disposeRepoStore(path)
    if (activePath.value === path) {
      const next = tabs.value[Math.min(idx, tabs.value.length - 1)]
      activePath.value = next?.path ?? null
      if (next) activate(next.path)
    }
    persistSession()
  }

  async function reopenClosed() {
    const p = closedTabs.value.pop()
    if (p) await openRepo(p)
  }

  function moveTab(from: number, to: number) {
    if (from === to || from < 0 || to < 0 || from >= tabs.value.length || to >= tabs.value.length) return
    const [t] = tabs.value.splice(from, 1)
    tabs.value.splice(to, 0, t)
    persistSession()
  }

  function showHome() {
    navEpoch++
    activePath.value = null
    persistSession()
  }

  return {
    info, settings, tabs, activePath, closedTabs, ready, online, opening, update, updateConfigured, updateChecking, checkForUpdate, activeTab, activeRepo, accounts, recent,
    init, reloadInfo, refreshSettings, openRepo, addResolved, closeTab, reopenClosed, moveTab, activate, showHome,
  }
})
