// Release notes shown in the app (footer → Novidades). Newest first.
// The release script also uses the first entry as the notes of the auto-update.

export interface ChangelogEntry {
  version: string
  date: string
  items: { type: 'new' | 'improved' | 'fixed'; text: string }[]
}

export const changelog: ChangelogEntry[] = [
  {
    version: '0.2.0',
    date: '2026-10-16',
    items: [
      { type: 'new', text: 'O app agora se chama Commito.' },
      { type: 'new', text: 'Atualização automática: quando houver versão nova, um botão laranja aparece no rodapé para instalar sem baixar o instalador de novo.' },
      { type: 'new', text: 'Emojis nos commits: shortcodes como :sparkles: e :bug: (GitHub/gitmoji) agora aparecem como ✨ e 🐛.' },
      { type: 'new', text: 'Botão de doação via PIX no rodapé.' },
      { type: 'new', text: 'Novidades da versão acessíveis pelo rodapé.' },
      { type: 'improved', text: 'Repositórios grandes (com muitas worktrees e node_modules) abrem muito mais rápido.' },
      { type: 'improved', text: 'Abas restauradas em paralelo, com indicador de carregamento.' },
      { type: 'fixed', text: 'Ao trocar de aba enquanto um repositório ainda estava abrindo, o app voltava para ele.' },
      { type: 'fixed', text: 'Alterações dentro de outras worktrees aninhadas não disparam mais atualizações na aba errada.' },
    ],
  },
  {
    version: '0.1.1',
    date: '2026-10-16',
    items: [{ type: 'new', text: 'Zoom da interface com Ctrl +, Ctrl −, Ctrl+0 e Ctrl + roda do mouse, lembrado entre sessões.' }],
  },
  {
    version: '0.1.0',
    date: '2026-10-16',
    items: [{ type: 'new', text: 'Primeira versão: grafo de commits, alterações e staging por linha, branches, merge/rebase/cherry-pick, stash, worktrees, tags e contas GitHub/GitLab/Bitbucket.' }],
  },
]
