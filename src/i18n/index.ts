// Minimal i18n layer: pt-BR is the default; other locales can be added as dictionaries.
import { ref } from 'vue'
import ptBR from './pt-BR'

type Dict = Record<string, string>
const dictionaries: Record<string, Dict> = { 'pt-BR': ptBR }

export const locale = ref('pt-BR')

/** Translate `key`, interpolating `{name}` placeholders. Falls back to the key itself. */
export function t(key: string, params?: Record<string, string | number>): string {
  const dict = dictionaries[locale.value] ?? ptBR
  let s = dict[key] ?? ptBR[key] ?? key
  if (params) for (const [k, v] of Object.entries(params)) s = s.replaceAll(`{${k}}`, String(v))
  return s
}

export function registerLocale(code: string, dict: Dict) {
  dictionaries[code] = dict
}
