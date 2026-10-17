// Converts GitHub/gitmoji shortcodes (":sparkles:") into emoji, as GitHub and GitLab render them.
import { gemoji } from 'gemoji'

const byName = new Map<string, string>()
for (const e of gemoji) for (const n of e.names) byName.set(n, e.emoji)

const SHORTCODE = /:([a-z0-9_+-]+):/gi

/** Replaces known `:shortcode:` tokens; unknown ones (and times like 10:30:00) are kept as-is. */
export function emojify(text: string): string {
  if (!text || text.indexOf(':') === -1) return text
  return text.replace(SHORTCODE, (m, name: string) => byName.get(name.toLowerCase()) ?? m)
}
