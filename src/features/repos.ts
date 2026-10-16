// Repository lifecycle flows (open / init / clone) shared by the home screen, toolbar and palette.
import { open } from '@tauri-apps/plugin-dialog'
import { useAppStore } from '@/stores/app'
import { useUiStore } from '@/stores/ui'
import CloneDialog from '@/components/dialogs/CloneDialog.vue'
import InitDialog from '@/components/dialogs/InitDialog.vue'

export async function openRepoFolder() {
  const app = useAppStore()
  const dir = await open({ directory: true, title: 'Abrir repositório Git' })
  if (typeof dir === 'string') await app.openRepo(dir)
}

export async function cloneRepo(prefill: { url?: string; accountId?: string | null; name?: string } = {}) {
  const ui = useUiStore()
  await ui.openDialog(CloneDialog, prefill)
}

export async function initRepo() {
  const ui = useUiStore()
  await ui.openDialog(InitDialog, {})
}
