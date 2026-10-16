// Client-side pre-validation of ref names (the backend runs `git check-ref-format` too).
export function refNameError(name: string): string | null {
  const n = name.trim()
  if (!n) return 'Informe um nome.'
  if (/\s/.test(n)) return 'Não use espaços.'
  if (n.startsWith('-')) return 'Não pode começar com "-".'
  if (/[~^:?*\[\\]/.test(n) || n.includes('..') || n.includes('@{')) return 'Caracteres inválidos (~ ^ : ? * [ \\ .. @{).'
  if (n.startsWith('/') || n.endsWith('/') || n.endsWith('.') || n.endsWith('.lock') || n.includes('//')) return 'Formato inválido.'
  return null
}

/** Suggests a branch name from free text (e.g. a commit summary). */
export function slugifyBranch(text: string): string {
  return text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9/._-]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
    .slice(0, 60)
}
