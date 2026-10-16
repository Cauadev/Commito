// Typed bridge to the Rust backend. All `invoke` calls go through here.
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AccountProfile, AppError, AppInfo, Blame, CheckoutTarget, CloneRequest, CommitDetail, CommitRequest,
  ConflictResolution, DiffSource, FileChange, FileDiff, GitRef, GitStatus, GraphPayload, GraphScope,
  HistoryEntry, HunkSelection, Identity, LineAction, MergeMode, OperationResult, ProgressPayload, Provider,
  ProviderAccount, PullStrategy, PushRequest, Remote, RepoChangedPayload, RepoPage, ResetMode, ResolvedRepo,
  SeqAction, Settings, SshDiagnostics, StashEntry, StashRequest, Submodule, TodoItem, UndoView, Workspace,
  Worktree, WorktreeRequest,
} from '@/types/git'

export function isAppError(e: unknown): e is AppError {
  return typeof e === 'object' && e !== null && 'code' in e && 'message' in e
}

export function toAppError(e: unknown): AppError {
  if (isAppError(e)) return e
  if (e instanceof Error) return { code: 'internal', message: e.message }
  return { code: 'internal', message: String(e) }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    throw toAppError(e)
  }
}

export const newOpId = () => `op-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

export const api = {
  // system
  appInfo: () => call<AppInfo>('app_info'),
  setGitPath: (path: string | null) => call<void>('set_git_path', { path }),
  getSettings: () => call<Settings>('get_settings'),
  saveUiSettings: (ui: Record<string, unknown>) => call<void>('save_ui_settings', { ui }),
  saveSession: (tabs: string[], active: string | null) => call<void>('save_session', { tabs, active }),
  setPreferences: (cloneDir: string | null, terminal: string | null) => call<void>('set_preferences', { cloneDir, terminal }),
  removeRecent: (path: string) => call<void>('remove_recent', { path }),
  defaultCloneDir: () => call<string>('default_clone_dir'),
  cancelOperation: (opId: string) => call<boolean>('cancel_operation', { opId }),
  openInExplorer: (path: string) => call<void>('open_in_explorer', { path }),
  openRepoFile: (repo: string, path: string) => call<void>('open_repo_file', { repo, path }),
  openUrl: (url: string) => call<void>('open_url', { url }),
  openTerminal: (path: string) => call<void>('open_terminal', { path }),
  sshDiagnostics: () => call<SshDiagnostics>('ssh_diagnostics'),

  // repository
  openRepository: (path: string) => call<ResolvedRepo>('open_repository', { path }),
  closeRepository: (path: string) => call<void>('close_repository', { path }),
  initRepository: (path: string, initialBranch: string | null) => call<ResolvedRepo>('init_repository', { path, initialBranch }),
  cloneRepository: (request: CloneRequest, opId: string, accountId: string | null) =>
    call<ResolvedRepo>('clone_repository', { request, opId, accountId }),
  getStatus: (repo: string, includeIgnored = false) => call<GitStatus>('get_status', { repo, includeIgnored }),
  getGraph: (repo: string, scope: GraphScope, limit: number, includeWip: boolean, reset = false) =>
    call<GraphPayload>('get_graph', { repo, scope, limit, includeWip, reset }),
  getRefs: (repo: string) => call<GitRef[]>('get_refs', { repo }),
  searchCommits: (repo: string, query: string, scope: GraphScope) => call<string[]>('search_commits', { repo, query, scope }),
  getCommit: (repo: string, sha: string) => call<CommitDetail>('get_commit', { repo, sha }),
  getStashUntracked: (repo: string, hash: string) => call<FileChange[]>('get_stash_untracked', { repo, hash }),
  getWorkingDiff: (repo: string, path: string, source: DiffSource, context = 3) =>
    call<FileDiff>('get_working_diff', { repo, path, source, context }),
  getCommitDiff: (repo: string, base: string | null, sha: string, path: string, context = 3) =>
    call<FileDiff>('get_commit_diff', { repo, base, sha, path, context }),
  compareRefs: (repo: string, base: string, target: string) => call<FileChange[]>('compare_refs', { repo, base, target }),
  getFileHistory: (repo: string, path: string) => call<HistoryEntry[]>('get_file_history', { repo, path }),
  getBlame: (repo: string, path: string, rev: string | null) => call<Blame>('get_blame', { repo, path, rev }),
  rebasePlan: (repo: string, base: string) => call<HistoryEntry[]>('rebase_plan', { repo, base }),
  readWorktreeFile: (repo: string, path: string) => call<string>('read_worktree_file', { repo, path }),
  getRemotes: (repo: string) => call<Remote[]>('get_remotes', { repo }),
  getWorktrees: (repo: string) => call<Worktree[]>('get_worktrees', { repo }),
  getStashes: (repo: string) => call<StashEntry[]>('get_stashes', { repo }),
  getIdentity: (repo: string) => call<Identity>('get_identity', { repo }),
  getSubmodules: (repo: string) => call<Submodule[]>('get_submodules', { repo }),
  getRepoAuth: (repo: string) => call<string | null>('get_repo_auth', { repo }),
  setRepoAuth: (repo: string, accountId: string | null) => call<void>('set_repo_auth', { repo, accountId }),
  listUndo: (repo: string) => call<UndoView[]>('list_undo', { repo }),
  undoLast: (repo: string, id: string) => call<string>('undo_last', { repo, id }),

  // index & commit
  stage: (repo: string, paths: string[]) => call<OperationResult>('stage_files', { repo, paths }),
  unstage: (repo: string, paths: string[]) => call<OperationResult>('unstage_files', { repo, paths }),
  applyLines: (repo: string, path: string, action: LineAction, selections: HunkSelection[], context = 3) =>
    call<OperationResult>('apply_lines', { repo, path, action, selections, context }),
  discard: (repo: string, tracked: string[], untracked: string[]) => call<OperationResult>('discard_changes', { repo, tracked, untracked }),
  commit: (repo: string, request: CommitRequest) => call<OperationResult>('commit', { repo, request }),
  lastCommitMessage: (repo: string) => call<string>('last_commit_message', { repo }),
  setIdentity: (repo: string, global: boolean, name: string, email: string) => call<void>('set_identity', { repo, global, name, email }),
  setPullStrategy: (repo: string, strategy: string, global: boolean) => call<void>('set_pull_strategy', { repo, strategy, global }),

  // branches & history
  createBranch: (repo: string, name: string, start: string | null, checkout: boolean) =>
    call<OperationResult>('create_branch', { repo, name, start, checkout }),
  checkout: (repo: string, target: CheckoutTarget, autoStash = false) => call<OperationResult>('checkout', { repo, target, autoStash }),
  renameBranch: (repo: string, old: string, newName: string) => call<OperationResult>('rename_branch', { repo, old, new: newName }),
  deleteBranch: (repo: string, name: string, force: boolean) => call<OperationResult>('delete_branch', { repo, name, force }),
  setUpstream: (repo: string, branch: string, upstream: string | null) => call<OperationResult>('set_upstream', { repo, branch, upstream }),
  merge: (repo: string, target: string, mode: MergeMode) => call<OperationResult>('merge', { repo, target, mode }),
  rebase: (repo: string, onto: string) => call<OperationResult>('rebase', { repo, onto }),
  rebaseInteractive: (repo: string, base: string, todo: TodoItem[]) => call<OperationResult>('rebase_interactive', { repo, base, todo }),
  sequencer: (repo: string, action: SeqAction) => call<OperationResult>('sequencer', { repo, action }),
  resolveConflicts: (repo: string, paths: string[], resolution: ConflictResolution) =>
    call<OperationResult>('resolve_conflicts', { repo, paths, resolution }),
  cherryPick: (repo: string, shas: string[], noCommit = false) => call<OperationResult>('cherry_pick', { repo, shas, noCommit }),
  revert: (repo: string, sha: string, noCommit = false) => call<OperationResult>('revert_commit', { repo, sha, noCommit }),
  reset: (repo: string, rev: string, mode: ResetMode, confirmed = false) => call<OperationResult>('reset', { repo, rev, mode, confirmed }),

  // tags
  createTag: (repo: string, name: string, target: string, message: string | null) => call<OperationResult>('create_tag', { repo, name, target, message }),
  deleteTag: (repo: string, name: string) => call<OperationResult>('delete_tag', { repo, name }),
  renameTag: (repo: string, old: string, newName: string) => call<OperationResult>('rename_tag', { repo, old, new: newName }),

  // stash
  stashPush: (repo: string, request: StashRequest) => call<OperationResult>('stash_push', { repo, request }),
  stashApply: (repo: string, index: number, hash: string, pop: boolean, restoreIndex = false) =>
    call<OperationResult>('stash_apply', { repo, index, hash, pop, restoreIndex }),
  stashDrop: (repo: string, index: number, hash: string) => call<OperationResult>('stash_drop', { repo, index, hash }),
  stashClear: (repo: string) => call<OperationResult>('stash_clear', { repo }),
  stashBranch: (repo: string, index: number, hash: string, name: string) => call<OperationResult>('stash_branch', { repo, index, hash, name }),

  // worktrees
  worktreeAdd: (repo: string, request: WorktreeRequest) => call<OperationResult>('worktree_add', { repo, request }),
  worktreeRemove: (repo: string, path: string, force: boolean) => call<OperationResult>('worktree_remove', { repo, path, force }),
  worktreePrune: (repo: string) => call<OperationResult>('worktree_prune', { repo }),
  worktreeLock: (repo: string, path: string, lock: boolean, reason: string | null) => call<OperationResult>('worktree_lock', { repo, path, lock, reason }),

  // remotes
  remoteAdd: (repo: string, name: string, url: string) => call<OperationResult>('remote_add', { repo, name, url }),
  remoteSetUrl: (repo: string, name: string, url: string, push: boolean) => call<OperationResult>('remote_set_url', { repo, name, url, push }),
  remoteRename: (repo: string, old: string, newName: string) => call<OperationResult>('remote_rename', { repo, old, new: newName }),
  remoteRemove: (repo: string, name: string) => call<OperationResult>('remote_remove', { repo, name }),
  fetch: (repo: string, remote: string | null, prune: boolean, opId: string) => call<OperationResult>('fetch', { repo, remote, prune, opId }),
  pull: (repo: string, strategy: PullStrategy, opId: string) => call<OperationResult>('pull', { repo, strategy, opId }),
  push: (repo: string, request: PushRequest, opId: string) => call<OperationResult>('push', { repo, request, opId }),
  deleteRemoteBranch: (repo: string, remote: string, branch: string, opId: string) =>
    call<OperationResult>('delete_remote_branch', { repo, remote, branch, opId }),
  pushTag: (repo: string, remote: string, tag: string | null, opId: string) => call<OperationResult>('push_tag', { repo, remote, tag, opId }),
  deleteRemoteTag: (repo: string, remote: string, tag: string, opId: string) => call<OperationResult>('delete_remote_tag', { repo, remote, tag, opId }),
  pruneRemote: (repo: string, remote: string, opId: string) => call<OperationResult>('prune_remote', { repo, remote, opId }),
  testRemote: (repo: string, remote: string, accountId: string | null) => call<OperationResult>('test_remote', { repo, remote, accountId }),
  submoduleUpdate: (repo: string, opId: string) => call<OperationResult>('submodule_update', { repo, opId }),

  // accounts
  listAccounts: () => call<ProviderAccount[]>('list_accounts'),
  addAccount: (provider: Provider, token: string, baseUrl: string | null, email: string | null, label: string | null) =>
    call<{ account: ProviderAccount; warnings: string[] }>('add_account', { provider, token, baseUrl, email, label }),
  testAccount: (id: string) => call<AccountProfile>('test_account', { id }),
  removeAccount: (id: string) => call<void>('remove_account', { id }),
  listRemoteRepos: (id: string, page: number, query: string | null, workspace: string | null) =>
    call<RepoPage>('list_remote_repos', { id, page, query, workspace }),
  listWorkspaces: (id: string) => call<Workspace[]>('list_workspaces', { id }),
  tokenHelp: (provider: Provider) => call<string>('token_help', { provider }),
}

export const events = {
  onProgress: (cb: (p: ProgressPayload) => void): Promise<UnlistenFn> => listen<ProgressPayload>('fg://progress', (e) => cb(e.payload)),
  onRepoChanged: (cb: (p: RepoChangedPayload) => void): Promise<UnlistenFn> =>
    listen<RepoChangedPayload>('fg://repo-changed', (e) => cb(e.payload)),
}
