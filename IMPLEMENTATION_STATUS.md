# ForgeGraph — status da implementação

Legenda: ✅ implementado **e verificado** (teste automatizado e/ou execução real do app) · 🟡 implementado, verificação parcial · ⛔ não implementado.

Verificação real feita com o app rodando (`pnpm tauri dev`) e dirigido via WebView2/CDP contra repositórios reais: um repositório de demonstração com branches, merges, tags, stash e um remote *bare* local, e um repositório sintético de 30.001 commits / 858 branches.

## Testes automatizados

- `pnpm test:rust` → **43 testes** (42 na lib + 1 de integração com o executável): parsers (status porcelain v2, numstat, log, refs, worktrees, stash, blame, diff), algoritmo de lanes, construção de patches parciais, classificação de erros, redação de segredos, validação de URLs/caminhos, settings, **fluxos Git reais em repositórios temporários** (grafo com merge, staging/unstaging por linha, conflito de cherry-pick + abort, conflito de merge + theirs + continue, stash com untracked, worktrees, reset/discard, proteção contra injection) e **cofre do Windows + ponte GIT_ASKPASS com o executável real**.
- `pnpm test` → testes do frontend (validação de nomes de ref, helpers).
- `pnpm build` → `vue-tsc --noEmit` (TypeScript strict) sem erros.

## Funcionalidades

| Área | Status | Observações |
|---|---|---|
| App desktop Tauri 2, três painéis redimensionáveis, abas, toolbar, statusbar | ✅ | Tamanhos dos painéis e colunas persistidos; sidebar recolhível; diff maximizável. |
| Tela inicial com diagnóstico do Git/GCM/LFS e onboarding | ✅ | Caminho do git.exe configurável. |
| Abrir / inicializar / clonar (HTTPS/SSH), abas restauradas | ✅ abrir/abas · 🟡 clone | Clone usa o mesmo runner com progresso/cancelamento; testado em teste de unidade de URL, não contra um provedor real nesta sessão. |
| Grafo DAG real (lanes, merges, octopus, cores estáveis, WIP e stash como nós) | ✅ | 30k commits: 1ª página 0,9 s, histórico completo ~2 s (build debug), rolagem ~73 fps. |
| Escopos All/Current/Local/Remote, filtro por mensagem/autor/SHA, busca no histórico completo, seleção múltipla | ✅ | |
| Status, stage/unstage de arquivo, hunk e linha, discard (untracked → Lixeira) | ✅ | |
| Diff unificado e lado a lado, realce de sintaxe, binários, contexto ajustável | ✅ | |
| Commit, amend, sign-off, `--no-verify`, identidade global/local | ✅ | Assinatura GPG/SSH respeita `commit.gpgsign` existente. |
| Branches: criar, checkout (auto-stash opcional), renomear, excluir (local/remoto), upstream | ✅ | Checkout com upstream remoto cria branch local rastreando. |
| Fetch / Pull (merge, rebase, ff-only, config) / Push (`-u`, `--force-with-lease` com confirmação digitada) | ✅ | Push e fetch verificados contra remote bare local; ahead/behind atualizados. |
| Remotes: adicionar, editar URL fetch/push, renomear, remover, prune, testar conexão | 🟡 | Backend testado; fluxo de UI exercitado parcialmente. |
| Merge (padrão/no-ff/ff-only/squash), rebase, rebase interativo (pick/squash/fixup/drop/reordenar) | ✅ merge · 🟡 rebase | Merge com conflito → resolver → Continuar verificado na UI. |
| Conflitos: seção dedicada, marcadores, ours/theirs/marcar resolvido, Continue/Skip/Abort | ✅ | |
| Cherry-pick (um ou vários, ordem cronológica), revert, reset soft/mixed/keep/hard | ✅ backend · 🟡 UI | Reset hard exige confirmação digitada e flag no backend. |
| Stash: criar (com untracked/keep-index/por arquivo), aplicar, pop, drop, clear, branch, ver arquivos | ✅ | |
| Worktrees: listar (limpa/suja/locked/prunable), criar (branch existente/nova/detached), abrir em aba, remover (força só com confirmação), prune, lock/unlock | ✅ | Verificado: criação abre nova aba e aparece na sidebar; Git impede a mesma branch em duas worktrees. |
| Tags: criar (leve/anotada), excluir local/remota, enviar, renomear por recriação | 🟡 | Backend coberto; UI não exercitada ponta a ponta. |
| Blame, histórico de arquivo, comparação entre commits/branches | 🟡 | Backend testado (blame/histórico em teste de integração). |
| Undo honesto | ✅ | Apenas operações locais reversíveis feitas no app, validando o estado atual. |
| Watcher (mudanças por IDE/terminal) | ✅ | Commits feitos via CLI apareceram sem recarregar. |
| Paleta `Ctrl+K`, atalhos, menus de contexto em commits/branches/tags/stashes/worktrees/arquivos/abas | ✅ | |
| Submódulos (listar/atualizar), repositórios bare, worktrees em abas isoladas | 🟡 | |
| Tema escuro (padrão) e claro | ✅ escuro · 🟡 claro | |
| i18n | 🟡 | Camada `t()` com dicionário pt-BR usada no shell; parte dos textos ainda está inline em pt-BR. |

## Provedores

| Item | Status | Observações |
|---|---|---|
| GitHub (PAT clássico/fine-grained, GHE via URL base) | 🟡 | Chamada real à API verificada (401 tratado corretamente); **não testado com um token válido** nesta sessão. |
| GitLab (PAT, self-managed HTTPS) | 🟡 | Idem. |
| Bitbucket Cloud (e-mail Atlassian + API token com escopos; `/user/workspaces` + `/repositories/{ws}`) | 🟡 | Idem. App Passwords não são usadas. |
| Tokens no Windows Credential Manager; nunca em settings/URL/logs/WebView | ✅ | Teste com o cofre real; redação de segredos testada. |
| Git HTTPS usando token da conta via GIT_ASKPASS (apenas mesmo host) | ✅ ponte · 🟡 push real | Ponte askpass verificada com o executável; push contra provedor real não testado. |
| Bloqueio de redirect para outro host | ✅ | Política customizada do reqwest. |
| OAuth (PKCE) | ⛔ | Não implementado (exige client ID registrado); nenhum botão de OAuth é exibido. |
| Pull/Merge Requests | ⛔ | Opcional na especificação; não implementado. |

## Distribuição

| Item | Status |
|---|---|
| `bundle.targets = ["nsis"]`, instalação por usuário, pt-BR/inglês, bootstrapper do WebView2, ícones próprios | ✅ configurado |
| Instalador gerado | ✅ `src-tauri/target/release/bundle/nsis/ForgeGraph_0.1.0_x64-setup.exe` (2,65 MiB). Verificado: instalação silenciosa por usuário em `%LOCALAPPDATA%\ForgeGraph`, atalho no Menu Iniciar, entrada de desinstalação, app instalado abriu repositórios reais (grafo, diff), desinstalação limpa. |
| Assinatura de código | ⛔ (configurável no futuro; SmartScreen pode alertar) |
| Git portátil embutido no instalador | ⛔ (usa o Git for Windows instalado; onboarding explica a dependência) |

## Limitações conhecidas

- Integrações GitHub/GitLab/Bitbucket precisam ser validadas por você com tokens reais (fluxo: Contas → Conectar → Repositórios → Clonar → fetch/pull/push).
- Prompts interativos de SSH (passphrase sem agente, aceitar host key) não aparecem dentro do app: use o `ssh-agent` e aceite a chave do host uma vez pelo terminal.
- Reword no rebase interativo não é suportado (pick/squash/fixup/drop/reordenar sim).
- Merge `--squash` com conflito não tem “Abortar” no app (o Git não registra operação em andamento); resolva ou use `git reset --merge`.
- Sem visualização de imagens no diff (binários são apenas sinalizados).
