// Syntax highlighting with a curated subset of highlight.js languages (keeps the bundle small).
import hljs from 'highlight.js/lib/core'
import bash from 'highlight.js/lib/languages/bash'
import c from 'highlight.js/lib/languages/c'
import cpp from 'highlight.js/lib/languages/cpp'
import csharp from 'highlight.js/lib/languages/csharp'
import css from 'highlight.js/lib/languages/css'
import dockerfile from 'highlight.js/lib/languages/dockerfile'
import go from 'highlight.js/lib/languages/go'
import ini from 'highlight.js/lib/languages/ini'
import java from 'highlight.js/lib/languages/java'
import javascript from 'highlight.js/lib/languages/javascript'
import json from 'highlight.js/lib/languages/json'
import kotlin from 'highlight.js/lib/languages/kotlin'
import markdown from 'highlight.js/lib/languages/markdown'
import php from 'highlight.js/lib/languages/php'
import powershell from 'highlight.js/lib/languages/powershell'
import python from 'highlight.js/lib/languages/python'
import ruby from 'highlight.js/lib/languages/ruby'
import rust from 'highlight.js/lib/languages/rust'
import scss from 'highlight.js/lib/languages/scss'
import sql from 'highlight.js/lib/languages/sql'
import swift from 'highlight.js/lib/languages/swift'
import typescript from 'highlight.js/lib/languages/typescript'
import xml from 'highlight.js/lib/languages/xml'
import yaml from 'highlight.js/lib/languages/yaml'

const langs: Record<string, unknown> = {
  bash, c, cpp, csharp, css, dockerfile, go, ini, java, javascript, json, kotlin, markdown, php, powershell, python,
  ruby, rust, scss, sql, swift, typescript, xml, yaml,
}
for (const [name, def] of Object.entries(langs)) hljs.registerLanguage(name, def as Parameters<typeof hljs.registerLanguage>[1])

const byExt: Record<string, string> = {
  ts: 'typescript', tsx: 'typescript', mts: 'typescript', cts: 'typescript',
  js: 'javascript', jsx: 'javascript', mjs: 'javascript', cjs: 'javascript',
  vue: 'xml', svelte: 'xml', html: 'xml', htm: 'xml', xml: 'xml', svg: 'xml', csproj: 'xml', xaml: 'xml',
  json: 'json', jsonc: 'json', css: 'css', scss: 'scss', sass: 'scss', less: 'scss',
  rs: 'rust', py: 'python', java: 'java', kt: 'kotlin', kts: 'kotlin', cs: 'csharp', go: 'go', rb: 'ruby',
  php: 'php', swift: 'swift', c: 'c', h: 'c', cpp: 'cpp', cc: 'cpp', hpp: 'cpp', cxx: 'cpp',
  sql: 'sql', sh: 'bash', bash: 'bash', zsh: 'bash', ps1: 'powershell', psm1: 'powershell',
  yml: 'yaml', yaml: 'yaml', toml: 'ini', ini: 'ini', cfg: 'ini', env: 'ini', md: 'markdown', markdown: 'markdown',
}

export function languageFor(path: string): string | null {
  const name = path.split('/').pop()?.toLowerCase() ?? ''
  if (name === 'dockerfile' || name.startsWith('dockerfile.')) return 'dockerfile'
  const ext = name.includes('.') ? name.split('.').pop()! : ''
  return byExt[ext] ?? null
}

export function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

/** Highlights a single line. Falls back to escaped text on errors or very long lines. */
export function highlightLine(text: string, lang: string | null): string {
  if (!lang || text.length > 2000) return escapeHtml(text)
  try {
    return hljs.highlight(text, { language: lang, ignoreIllegals: true }).value
  } catch {
    return escapeHtml(text)
  }
}
