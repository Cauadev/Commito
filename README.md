# ForgeGraph

Cliente Git desktop para Windows, com o histórico em forma de grafo (branches e merges em lanes coloridas), worktrees, stash, cherry-pick, rebase interativo e integração com **GitHub**, **GitLab** e **Bitbucket Cloud** — inclusive repositórios privados. Feito com **Tauri 2 + Vue 3 + TypeScript + shadcn-vue + Tailwind**, com o backend em **Rust**.

> O app usa o **Git instalado no Windows** para todas as operações. Nada é simulado: status, grafo, diffs, commits, merges e pushes são do seu repositório real.

---

## Instalação (usuário final)

1. Instale o **Git for Windows** (inclui o Git Credential Manager): <https://git-scm.com/download/win>. Versão recomendada: 2.36 ou superior.
2. Execute o instalador `ForgeGraph_0.1.0_x64-setup.exe` (veja abaixo onde ele é gerado).
   - Instala por usuário (sem precisar de administrador), com atalho no Menu Iniciar e desinstalador em *Aplicativos instalados*.
   - Se o **WebView2** não estiver presente (raro no Windows 10/11 atualizados), o instalador baixa e instala automaticamente — requer internet nesse momento.
3. O executável não é assinado digitalmente: o **SmartScreen** pode exibir “O Windows protegeu o computador”. Clique em *Mais informações → Executar assim mesmo*. Isso não é um defeito do app; a assinatura de código pode ser configurada no futuro (`bundle.windows.certificateThumbprint` ou `signCommand` no `tauri.conf.json`).

Ao abrir, a tela inicial verifica o ambiente (versão do Git, Git Credential Manager, Git LFS). Se o Git não for encontrado, ela mostra como instalar ou permite informar o caminho do `git.exe` em **Configurações → Git**.

## Desenvolvimento

### Pré-requisitos

| Ferramenta | Observação |
|---|---|
| Node.js 20+ e **pnpm** | `npm i -g pnpm` |
| **Rust** (toolchain `stable-x86_64-pc-windows-msvc`) | <https://rustup.rs> |
| **Visual Studio Build Tools** | carga de trabalho “Desenvolvimento para desktop com C++” (MSVC + Windows SDK) |
| **WebView2 Runtime** | já incluso no Windows 10/11 atualizado |
| **Git for Windows** (com GCM) | necessário em tempo de execução |

### Comandos

```powershell
pnpm install            # dependências do frontend
pnpm tauri dev          # app desktop em modo desenvolvimento (Vite na porta 1430)
pnpm build              # type-check (vue-tsc) + build do frontend
pnpm test:rust          # testes Rust (parsers, grafo, fluxos Git reais, cofre/askpass)
pnpm tauri build        # build de release + instalador NSIS
```

### Onde fica o instalador

```
src-tauri\target\release\bundle\nsis\ForgeGraph_0.1.0_x64-setup.exe
```

O executável do app (sem instalador) fica em `src-tauri\target\release\forgegraph.exe`.

## Uso rápido

- **Abrir / Clonar / Inicializar** pela tela inicial (ou `Ctrl+K`). Cada repositório ou worktree abre em uma aba própria; as abas são restauradas ao reabrir o app.
- **Grafo** (centro): clique seleciona; `Ctrl+clique`/`Shift+clique` seleciona vários commits (para cherry-pick em lote ou comparação); duplo clique numa branch faz checkout; botão direito abre o menu de ações (checkout, branch, tag, cherry-pick, revert, reset soft/mixed/hard, merge, rebase, rebase interativo, comparar, copiar SHA, abrir no navegador). Filtro por mensagem/autor/SHA com `Ctrl+F`; “Buscar no histórico” procura além do que está carregado.
- **Alterações** (direita, linha `// WIP`): Não preparados / Preparados em modo Path ou Tree; clique no arquivo abre o diff no centro. No diff é possível **preparar/remover/descartar um hunk** ou **selecionar linhas** (clique no número da linha, `Shift` para intervalo). Commit com resumo + descrição, Amend, Signed-off-by e opção de ignorar hooks (`Ctrl+Enter`).
- **Conflitos**: aparecem numa seção própria com “Nossa / Deles / Marcar resolvido”, visualização dos marcadores e banner com **Continuar / Pular / Abortar**.
- **Barra lateral**: branches locais e remotas (agrupadas por pasta), worktrees (estado limpo/sujo, bloqueio), stashes, tags, remotes e submódulos — todos com menu de contexto.
- **Desfazer**: só oferece operações locais comprovadamente reversíveis feitas no app (commit, amend, checkout, criar/excluir/renomear branch, tags, stash, merge/cherry-pick/revert/reset/pull locais) e apenas enquanto o repositório ainda está no estado deixado pela operação. Push nunca é “desfeito”.
- Atalhos: `Ctrl+K` comandos · `F5` atualizar · `Esc` fecha o diff · `Ctrl+W` fecha a aba · `Ctrl+Shift+T` reabre aba · `Ctrl+Tab` próxima aba · `Ctrl+Alt+F` filtro da barra lateral.

## Autenticação

O ForgeGraph separa dois assuntos:

### 1. Transporte Git (fetch / pull / push / clone)

- **HTTPS**: por padrão o Git usa o **Git Credential Manager** (login pelo navegador; credenciais no cofre do Windows). Nenhuma conta precisa estar conectada no app.
- **SSH**: o Git usa suas chaves, o `ssh-agent` do Windows, `~/.ssh/config` e `known_hosts`. O app nunca lê ou pede a chave privada. Em **Configurações → SSH / HTTPS** há um diagnóstico (agente ativo, chaves carregadas). Na primeira conexão a um host, aceite a chave do host pelo terminal (`ssh -T git@github.com`).
- **Opcional — token de uma conta conectada**: em **Configurações → Repositório** (ou ao clonar a partir de uma conta) você pode fazer o Git usar o token da conta. O token é lido do cofre pelo próprio executável do ForgeGraph atuando como `GIT_ASKPASS`; **ele nunca aparece em URLs, linhas de comando, logs ou na interface**, e só é enviado para remotes HTTPS do mesmo host da conta.
- “Testar conexão” no menu do remote executa `git ls-remote` para confirmar que as credenciais Git funcionam (validar a API não garante permissão de push).

### 2. Contas de API (listar e clonar repositórios)

Em **Contas** (barra superior):

| Provedor | O que informar | Escopos recomendados |
|---|---|---|
| **GitHub** (e GitHub Enterprise Server via URL base) | Personal Access Token. Gere em <https://github.com/settings/tokens> | Classic: `repo` (privados) e `read:org`. Fine-grained: acesso aos repositórios desejados, *Metadata: read*, *Contents: read/write* |
| **GitLab** (gitlab.com ou self-managed via URL base HTTPS) | Personal Access Token em *Preferences → Access tokens* | `read_api` (listar), `read_repository` / `write_repository` (Git com token) |
| **Bitbucket Cloud** | **E-mail da conta Atlassian** + **API token com escopos** (<https://id.atlassian.com/manage-profile/security/api-tokens>). *App Passwords foram descontinuadas e não são usadas.* | `read:user:bitbucket`, `read:workspace:bitbucket`, `read:repository:bitbucket`, `write:repository:bitbucket` |

- Os tokens ficam **somente** no **Gerenciador de Credenciais do Windows** (entrada `ForgeGraph`/`account:<id>`). O arquivo de configurações guarda apenas metadados (usuário, provedor, URL base).
- Várias contas por provedor são suportadas (use o campo “Rótulo”).
- Desconectar remove apenas a entrada do ForgeGraph; credenciais do GCM e de outros apps não são tocadas.
- Bitbucket: os workspaces vêm de `GET /2.0/user/workspaces` e os repositórios de `GET /2.0/repositories/{workspace}` (os endpoints antigos foram removidos pela Atlassian). Git HTTPS com token usa o usuário estático `x-bitbucket-api-token-auth`.
- Redirecionamentos HTTP para outro host são bloqueados, para que tokens nunca sejam enviados a um servidor diferente.

## Arquitetura

```
src/                         Frontend (Vue 3 + TS strict + Pinia + shadcn-vue)
  components/graph/          Grafo virtualizado (SVG por linha), badges de refs
  components/changes/        Alterações, lista Path/Tree, composer de commit
  components/diff/           Diff unificado/lado a lado com staging por linha, blame
  components/commit/         Detalhes de commit/stash, comparação
  components/sidebar/        Branches, remotes, worktrees, stashes, tags, submódulos
  components/dialogs/        Diálogos (clone, branch, merge, reset, worktree, contas…)
  features/actions.ts        Fluxos de UX com confirmações (compartilhados por menus/toolbar/paleta)
  stores/                    app (abas/sessão), repo (estado isolado por aba), ui (toasts/prefs)
  services/api.ts            Wrappers tipados de invoke + eventos
src-tauri/src/
  git/runner.rs              Execução central do git (sem shell, cancelamento, progresso, timeout)
  git/status|diff|log|refs   Parsers de formatos estáveis (porcelain v2 -z, numstat -z, for-each-ref…)
  git/graph.rs               Algoritmo determinístico de lanes do DAG
  git/read|ops|remote|undo   Consultas, operações locais, rede e undo
  providers.rs               GitHub / GitLab / Bitbucket (reqwest + TLS do Windows)
  credentials.rs             Cofre do Windows + ponte GIT_ASKPASS
  watcher.rs                 Watcher de arquivos com debounce
  commands/                  Superfície invoke do Tauri
```

Decisões técnicas:

- **Git CLI** como único motor (sem `libgit2`), para compatibilidade total com hooks, LFS, assinatura, `credential.helper`, `core.sshCommand`, proxies e configurações do usuário.
- O grafo **não** usa `git log --graph`: o layout é calculado em Rust a partir dos pais de cada commit, com paginação e cache invalidado por mudança de refs/HEAD.
- **Staging parcial** reconstrói o patch no backend a partir dos bytes originais do diff (preserva encoding e CRLF) e rejeita seleções de um diff desatualizado.
- **Diff**: visualizador próprio (unificado e lado a lado) com realce de sintaxe via highlight.js em vez do Monaco — permite selecionar hunks/linhas para staging, mantém o bundle leve e casa com o tema.
- Escritas são **serializadas por repositório/worktree**; leituras rodam em paralelo com `GIT_OPTIONAL_LOCKS=0`.
- Arquivos não rastreados descartados vão para a **Lixeira** (recuperáveis), não são apagados.
- Interface em pt-BR com camada de i18n (`src/i18n`) pronta para outros idiomas.

## Solução de problemas

| Sintoma | O que fazer |
|---|---|
| “Git não encontrado” | Instale o Git for Windows ou informe o caminho em Configurações → Git. |
| Push/fetch HTTPS falha com autenticação | Confirme o login do GCM (`git credential-manager diagnose` no terminal) ou associe uma conta conectada ao repositório. |
| SSH: “Host key verification failed” | Rode `ssh -T git@<host>` uma vez no terminal para aceitar a chave do host. |
| SSH: “Permission denied (publickey)” | Inicie o serviço *OpenSSH Authentication Agent* e rode `ssh-add`; verifique a chave cadastrada no provedor. |
| “Outro processo Git está usando este repositório (.lock)” | Aguarde o outro processo; se nenhum estiver rodando, remova o `.git/index.lock` indicado nos detalhes. |
| Push rejeitado (non-fast-forward) | Faça Pull (merge ou rebase). Force push só com `--force-with-lease` e confirmação digitada. |
| Bitbucket 401 | Use o **e-mail Atlassian** (não o username) e um API token **com escopos**. |
| SmartScreen ao instalar | Executável não assinado — *Mais informações → Executar assim mesmo*. |

As configurações ficam em `%APPDATA%\dev.forgegraph.app\settings.json` (apenas preferências, abas, recentes e metadados de contas).

## Limitações

Veja [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md).
