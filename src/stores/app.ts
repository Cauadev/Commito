// Application-level state: environment info, settings, repository tabs and session restore.
import { defineStore } from 'pinia'
import { computed, ref, shallowRef } from 'vue'
import { api, events, toAppError } from '@/services/api'
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
    if (info.value?.gitFound && settings.value) {
      const toOpen = settings.value.openTabs ?? []
      const active = settings.value.activeTab
      for (const p of toOpen) {
        await openRepo(p, { activate: false, quiet: true })
      }
      const want = tabs.value.find((t) => active && t.path.toLowerCase() === active.toLowerCase()) ?? tabs.value[0]
      if (want) activate(want.path)
    }
  }

  async function reloadInfo() {
    info.value = await api.appInfo()
  }

  function persistSession() {
    api.saveSession(tabs.value.map((t) => t.path), activePath.value).catch(() => {})
  }

  function activate(path: string) {
    activePath.value = path
    persistSession()
    const t = tabs.value.find((x) => x.path === path)
    if (t) {
      const store = getRepoStore(t)
      if (!store.status) void store.loadAll()
    }
  }

  async function addResolved(resolved: ResolvedRepo, activateTab = true) {
    if (!tabs.value.some((t) => t.path.toLowerCase() === resolved.path.toLowerCase())) {
      tabs.value.push(resolved)
    }
    const existing = tabs.value.find((t) => t.path.toLowerCase() === resolved.path.toLowerCase())!
    if (activateTab) activate(existing.path)
    else persistSession()
    void refreshSettings()
    return existing
  }

  async function openRepo(path: string, opts: { activate?: boolean; quiet?: boolean } = {}) {
    try {
      const resolved = await api.openRepository(path)
      return await addResolved(resolved, opts.activate ?? true)
    } catch (e) {
      if (!opts.quiet) ui.error(toAppError(e), 'Não foi possível abrir o repositório')
      return null
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
    activePath.value = null
    persistSession()
  }

  return {
    info, settings, tabs, activePath, closedTabs, ready, online, activeTab, activeRepo, accounts, recent,
    init, reloadInfo, refreshSettings, openRepo, addResolved, closeTab, reopenClosed, moveTab, activate, showHome,
  }
})
