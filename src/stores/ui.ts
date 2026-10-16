// Toasts, operation progress, dialogs and persisted UI preferences.
import { defineStore } from 'pinia'
import { reactive, ref, shallowRef, watch, type Component } from 'vue'
import { api } from '@/services/api'
import type { AppError, ProgressPayload } from '@/types/git'

export type ToastKind = 'success' | 'error' | 'info' | 'warning'

export interface Toast {
  id: number
  kind: ToastKind
  title: string
  description?: string | null
  details?: string | null
  action?: { label: string; run: () => void }
  timeout: number
}

export interface RunningOp {
  opId: string
  label: string
  repo?: string | null
  phase: string
  percent: number | null
  line: string
  cancellable: boolean
  startedAt: number
}

export interface ConfirmOptions {
  title: string
  message: string
  details?: string
  confirmLabel?: string
  cancelLabel?: string
  destructive?: boolean
  /** Require typing this text to enable the confirm button. */
  typeToConfirm?: string
  checkbox?: string
}

export interface UiPrefs {
  theme: 'dark' | 'light'
  sidebarSize: number
  detailSize: number
  sidebarCollapsed: boolean
  diffMode: 'unified' | 'split'
  diffContext: number
  graphColumns: { refs: number; author: number; sha: number; date: number; showAuthor: boolean; showSha: boolean; showDate: boolean }
  fileView: 'path' | 'tree'
  graphScope: 'all' | 'current' | 'local' | 'remote'
  pullStrategy: 'config' | 'merge' | 'rebase' | 'ff_only'
  showWhitespace: boolean
  wordWrap: boolean
}

const defaultPrefs: UiPrefs = {
  theme: 'dark',
  sidebarSize: 18,
  detailSize: 27,
  sidebarCollapsed: false,
  diffMode: 'unified',
  diffContext: 3,
  graphColumns: { refs: 160, author: 124, sha: 64, date: 92, showAuthor: true, showSha: true, showDate: true },
  fileView: 'path',
  graphScope: 'all',
  pullStrategy: 'config',
  showWhitespace: false,
  wordWrap: false,
}

export const useUiStore = defineStore('ui', () => {
  const toasts = ref<Toast[]>([])
  const ops = reactive<Record<string, RunningOp>>({})
  const prefs = reactive<UiPrefs>(structuredClone(defaultPrefs))
  const prefsLoaded = ref(false)
  let nextId = 1

  function toast(kind: ToastKind, title: string, description?: string | null, extra: Partial<Toast> = {}) {
    const id = nextId++
    const timeout = extra.timeout ?? (kind === 'error' ? 12000 : kind === 'warning' ? 9000 : 4500)
    toasts.value.push({ id, kind, title, description, timeout, ...extra })
    if (toasts.value.length > 5) toasts.value.shift()
    if (timeout > 0) setTimeout(() => dismiss(id), timeout)
    return id
  }
  function dismiss(id: number) {
    toasts.value = toasts.value.filter((t) => t.id !== id)
  }
  function error(e: AppError, title?: string) {
    return toast('error', title ?? e.message, title ? e.message + (e.recovery ? ` ${e.recovery}` : '') : e.recovery, { details: e.details ?? null })
  }

  function startOp(opId: string, label: string, repo?: string | null, cancellable = true) {
    ops[opId] = { opId, label, repo, phase: '', percent: null, line: '', cancellable, startedAt: Date.now() }
  }
  function progress(p: ProgressPayload) {
    const op = ops[p.opId]
    if (!op) return
    op.phase = p.phase
    op.line = p.line
    if (p.percent != null) op.percent = p.percent
  }
  function endOp(opId: string) {
    delete ops[opId]
  }

  // ----------------------------------------------------------- dialogs
  const dialog = shallowRef<{ component: Component; props: Record<string, unknown>; resolve: (v: unknown) => void } | null>(null)

  function openDialog<T = unknown>(component: Component, props: Record<string, unknown> = {}): Promise<T | undefined> {
    return new Promise((resolve) => {
      dialog.value?.resolve(undefined)
      dialog.value = { component, props, resolve: resolve as (v: unknown) => void }
    })
  }
  function closeDialog(result?: unknown) {
    const d = dialog.value
    dialog.value = null
    d?.resolve(result)
  }

  const confirmState = shallowRef<{ opts: ConfirmOptions; resolve: (v: { ok: boolean; checked: boolean }) => void } | null>(null)
  function confirm(opts: ConfirmOptions): Promise<{ ok: boolean; checked: boolean }> {
    return new Promise((resolve) => {
      confirmState.value?.resolve({ ok: false, checked: false })
      confirmState.value = { opts, resolve }
    })
  }
  function resolveConfirm(ok: boolean, checked = false) {
    const c = confirmState.value
    confirmState.value = null
    c?.resolve({ ok, checked })
  }

  const detailsText = ref<{ title: string; text: string } | null>(null)
  const paletteOpen = ref(false)

  // ----------------------------------------------------------- prefs
  function loadPrefs(raw: Record<string, unknown> | undefined) {
    if (raw && typeof raw === 'object') {
      const p = raw.prefs as Partial<UiPrefs> | undefined
      if (p) {
        Object.assign(prefs, { ...structuredClone(defaultPrefs), ...p, graphColumns: { ...defaultPrefs.graphColumns, ...(p.graphColumns ?? {}) } })
      }
    }
    applyTheme()
    prefsLoaded.value = true
  }
  function applyTheme() {
    document.documentElement.classList.toggle('dark', prefs.theme === 'dark')
  }
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  watch(
    prefs,
    () => {
      applyTheme()
      if (!prefsLoaded.value) return
      clearTimeout(saveTimer)
      saveTimer = setTimeout(() => {
        api.saveUiSettings({ prefs: JSON.parse(JSON.stringify(prefs)) }).catch(() => {})
      }, 600)
    },
    { deep: true },
  )

  return {
    toasts, toast, dismiss, error,
    ops, startOp, progress, endOp,
    dialog, openDialog, closeDialog,
    confirmState, confirm, resolveConfirm,
    detailsText, paletteOpen,
    prefs, loadPrefs,
  }
})
