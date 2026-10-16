// DTOs mirroring the Rust backend (serde camelCase).

export type ErrorCode =
  | 'git_not_found' | 'not_a_repository' | 'repo_not_authorized' | 'git_failed' | 'conflict'
  | 'auth_failed' | 'host_key' | 'network' | 'non_fast_forward' | 'protected_branch'
  | 'dirty_worktree' | 'lock_file' | 'no_upstream' | 'cancelled' | 'timeout' | 'invalid_input'
  | 'io' | 'provider' | 'rate_limited' | 'credential' | 'not_found' | 'stale_state' | 'internal'

export interface AppError {
  code: ErrorCode
  message: string
  details?: string | null
  recovery?: string | null
  repo?: string | null
}

export interface ResolvedRepo {
  path: string
  name: string
  gitDir: string
  commonDir: string
  bare: boolean
  linkedWorktree: boolean
}

export type FileStatus =
  | 'modified' | 'added' | 'deleted' | 'renamed' | 'copied' | 'type_changed' | 'untracked' | 'ignored' | 'conflicted'

export interface FileChange {
  path: string
  origPath?: string | null
  status: FileStatus
  additions?: number | null
  deletions?: number | null
  binary: boolean
  submodule: boolean
  conflict?: string | null
}

export interface HeadInfo {
  oid?: string | null
  branch?: string | null
  detached: boolean
  unborn: boolean
  upstream?: string | null
  ahead?: number | null
  behind?: number | null
}

export type OperationKind = 'merge' | 'rebase' | 'cherry_pick' | 'revert' | 'bisect' | 'apply_mailbox'

export interface OperationState {
  kind: OperationKind
  headName?: string | null
  onto?: string | null
  step?: number | null
  total?: number | null
  currentCommit?: string | null
  message?: string | null
}

export interface GitStatus {
  head: HeadInfo
  staged: FileChange[]
  unstaged: FileChange[]
  conflicted: FileChange[]
  ignored: string[]
  stashCount: number
  operation?: OperationState | null
  bare: boolean
}

export type RefKind = 'local' | 'remote' | 'tag'

export interface GitRef {
  name: string
  short: string
  kind: RefKind
  target: string
  remote?: string | null
  upstream?: string | null
  ahead?: number | null
  behind?: number | null
  upstreamGone: boolean
  isHead: boolean
  worktreePath?: string | null
  annotated: boolean
  date: number
  message?: string | null
}

/** [kind (0 full, 1 top, 2 bottom), fromLane, toLane, color, dashed] */
export type GraphEdge = [number, number, number, number, number]

export type RowKind = 'commit' | 'wip' | 'stash'

export interface GraphRow {
  kind: RowKind
  hash: string
  parents: string[]
  subject: string
  authorName: string
  authorEmail: string
  authorTime: number
  committerTime: number
  lane: number
  color: number
  edges: GraphEdge[]
  stashRef?: string | null
}

export interface HeadRef {
  oid?: string | null
  branch?: string | null
  detached: boolean
}

export interface StashEntry {
  index: number
  reference: string
  hash: string
  base?: string | null
  message: string
  branch?: string | null
  time: number
  hasUntracked: boolean
}

export interface GraphPayload {
  rows: GraphRow[]
  refs: GitRef[]
  head: HeadRef
  hasMore: boolean
  maxLanes: number
  stashes: StashEntry[]
  totalLoaded: number
}

export type GraphScope = 'all' | 'current' | 'local' | 'remote'

export interface CommitDetail {
  hash: string
  parents: string[]
  authorName: string
  authorEmail: string
  authorTime: number
  committerName: string
  committerEmail: string
  committerTime: number
  subject: string
  body: string
  files: FileChange[]
  signature?: string | null
}

export type LineKind = 'context' | 'add' | 'del' | 'no_newline'

export interface DiffLine {
  kind: LineKind
  oldNo?: number | null
  newNo?: number | null
  content: string
}

export interface DiffHunk {
  header: string
  oldStart: number
  oldLines: number
  newStart: number
  newLines: number
  lines: DiffLine[]
}

export interface FileDiff {
  path: string
  oldPath?: string | null
  binary: boolean
  isNew: boolean
  isDeleted: boolean
  isRename: boolean
  oldMode?: string | null
  newMode?: string | null
  submodule: boolean
  hunks: DiffHunk[]
  truncated: boolean
  additions: number
  deletions: number
}

export type DiffSource = 'unstaged' | 'staged' | 'untracked'

export interface HunkSelection {
  hunk: number
  header: string
  lines?: number[] | null
}

export interface Worktree {
  path: string
  head?: string | null
  branch?: string | null
  bare: boolean
  detached: boolean
  locked: boolean
  lockReason?: string | null
  prunable: boolean
  prunableReason?: string | null
  isMain: boolean
  isCurrent: boolean
  dirty?: boolean | null
  changedFiles?: number | null
}

export interface Remote {
  name: string
  fetchUrl?: string | null
  pushUrl?: string | null
  webUrl?: string | null
}

export interface Identity {
  localName?: string | null
  localEmail?: string | null
  globalName?: string | null
  globalEmail?: string | null
  effectiveName?: string | null
  effectiveEmail?: string | null
  gpgSign: boolean
  signingKey?: string | null
  pullRebase?: string | null
  pullFf?: string | null
}

export interface Submodule {
  path: string
  hash: string
  state: string
  describe?: string | null
}

export interface OperationResult {
  ok: boolean
  message: string
  conflicts: boolean
  details?: string | null
  changed: string[]
}

export interface HistoryEntry {
  hash: string
  parents: string[]
  subject: string
  authorName: string
  authorEmail: string
  authorTime: number
}

export interface BlameLine { line: number; hash: string; content: string }
export interface BlameCommit { hash: string; author: string; time: number; summary: string }
export interface Blame { lines: BlameLine[]; commits: Record<string, BlameCommit> }

export interface UndoView {
  id: string
  label: string
  createdAt: number
  warning?: string | null
  available: boolean
  reason?: string | null
}

export type CheckoutTarget =
  | { kind: 'branch'; name: string }
  | { kind: 'remote'; remote_branch: string; local_name: string }
  | { kind: 'detached'; rev: string }

export type MergeMode = 'default' | 'no_ff' | 'ff_only' | 'squash'
export type PullStrategy = 'config' | 'merge' | 'rebase' | 'ff_only'
export type ResetMode = 'soft' | 'mixed' | 'hard' | 'keep'
export type SeqAction = 'continue' | 'abort' | 'skip'
export type LineAction = 'stage' | 'unstage' | 'discard'
export type ConflictResolution = 'ours' | 'theirs' | 'mark_resolved'

export interface CommitRequest {
  message: string
  amend?: boolean
  signOff?: boolean
  gpgSign?: boolean | null
  noVerify?: boolean
  allowEmpty?: boolean
}

export interface PushRequest {
  remote: string
  branch?: string | null
  remoteBranch?: string | null
  setUpstream?: boolean
  forceWithLease?: boolean
  tags?: boolean
  noVerify?: boolean
}

export interface StashRequest {
  message?: string | null
  includeUntracked?: boolean
  keepIndex?: boolean
  paths?: string[]
}

export interface WorktreeRequest {
  path: string
  branch?: string | null
  newBranch?: string | null
  start?: string | null
  detach?: boolean
}

export interface CloneRequest {
  url: string
  parentDir: string
  dirName: string
  branch?: string | null
  depth?: number | null
  recurseSubmodules?: boolean
}

export interface TodoItem { action: 'pick' | 'squash' | 'fixup' | 'drop'; hash: string }

// ----------------------------------------------------------------- app / providers

export type Provider = 'github' | 'gitlab' | 'bitbucket'

export interface ProviderAccount {
  id: string
  provider: Provider
  baseUrl?: string | null
  username: string
  displayName?: string | null
  avatarUrl?: string | null
  email?: string | null
  label?: string | null
  createdAt: number
  scopes?: string[] | null
  expiresAt?: string | null
}

export interface AccountProfile {
  username: string
  displayName?: string | null
  avatarUrl?: string | null
  scopes?: string[] | null
  expiresAt?: string | null
  warnings: string[]
}

export interface RemoteRepository {
  id: string
  provider: Provider
  name: string
  fullName: string
  owner: string
  description?: string | null
  private: boolean
  fork: boolean
  cloneHttps?: string | null
  cloneSsh?: string | null
  webUrl?: string | null
  defaultBranch?: string | null
  updatedAt?: string | null
}

export interface RepoPage { items: RemoteRepository[]; nextPage?: number | null }
export interface Workspace { slug: string; name: string }

export interface RecentRepo { path: string; name: string; lastOpened: number }

export interface Settings {
  version: number
  gitPath?: string | null
  recentRepos: RecentRepo[]
  openTabs: string[]
  activeTab?: string | null
  accounts: ProviderAccount[]
  repoAuth: Record<string, string>
  cloneDir?: string | null
  terminal?: string | null
  ui: Record<string, unknown>
}

export interface AppInfo {
  version: string
  gitFound: boolean
  gitPath?: string | null
  gitVersion?: string | null
  gitVersionOk: boolean
  gcmVersion?: string | null
  credentialHelper?: string | null
  lfsVersion?: string | null
  platform: string
  settingsPath: string
}

export interface SshDiagnostics {
  sshFound: boolean
  agentKeys: string[]
  agentError?: string | null
  configExists: boolean
  knownHostsExists: boolean
  publicKeys: string[]
}

export interface ProgressPayload {
  opId: string
  repo?: string | null
  phase: string
  percent?: number | null
  line: string
}

export interface RepoChangedPayload {
  repo: string
  kind: 'git' | 'worktree'
}
