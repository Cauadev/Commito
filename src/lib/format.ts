// Formatting helpers (pt-BR by default).
import { locale } from '@/i18n'

const rtf = () => new Intl.RelativeTimeFormat(locale.value, { numeric: 'auto' })

export function relativeTime(unixSeconds: number, now = Date.now()): string {
  const diff = unixSeconds * 1000 - now
  const abs = Math.abs(diff)
  const min = 60_000
  const hour = 60 * min
  const day = 24 * hour
  if (abs < min) return 'agora'
  if (abs < hour) return rtf().format(Math.round(diff / min), 'minute')
  if (abs < day) return rtf().format(Math.round(diff / hour), 'hour')
  if (abs < 30 * day) return rtf().format(Math.round(diff / day), 'day')
  if (abs < 365 * day) return rtf().format(Math.round(diff / (30 * day)), 'month')
  return rtf().format(Math.round(diff / (365 * day)), 'year')
}

export function absoluteTime(unixSeconds: number): string {
  return new Intl.DateTimeFormat(locale.value, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(unixSeconds * 1000))
}

export function shortTime(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000)
  const now = new Date()
  const sameYear = d.getFullYear() === now.getFullYear()
  return new Intl.DateTimeFormat(locale.value, sameYear ? { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' } : { day: '2-digit', month: 'short', year: 'numeric' }).format(d)
}

export const shortSha = (h: string | null | undefined, n = 7) => (h ? h.slice(0, n) : '')

export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean)
  if (!parts.length) return '?'
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase()
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase()
}

export function laneColor(i: number): string {
  return `var(--lane-${((i % 12) + 12) % 12})`
}

/** Stable hue from a string (author avatars). */
export function hashHue(s: string): number {
  let h = 0
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0
  return h % 360
}

export function basename(p: string): string {
  const parts = p.split(/[\\/]/)
  return parts[parts.length - 1] || p
}

export function dirname(p: string): string {
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'))
  return i >= 0 ? p.slice(0, i) : ''
}

export function plural(n: number, one: string, many: string) {
  return `${n} ${n === 1 ? one : many}`
}

export const statusLetter: Record<string, string> = {
  modified: 'M', added: 'A', deleted: 'D', renamed: 'R', copied: 'C', type_changed: 'T', untracked: 'U', ignored: 'I', conflicted: '!',
}

export const statusLabel: Record<string, string> = {
  modified: 'Modificado', added: 'Adicionado', deleted: 'Removido', renamed: 'Renomeado', copied: 'Copiado',
  type_changed: 'Tipo alterado', untracked: 'Não rastreado', ignored: 'Ignorado', conflicted: 'Em conflito',
}

export const statusClass: Record<string, string> = {
  modified: 'text-warning', added: 'text-added', deleted: 'text-removed', renamed: 'text-info', copied: 'text-info',
  type_changed: 'text-warning', untracked: 'text-added', ignored: 'text-muted-foreground', conflicted: 'text-destructive',
}
