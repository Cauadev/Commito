# API de doação (PIX via Asaas)

Pequeno serviço para rodar na sua VPS. O Commito chama `GET <URL>/pix?value=10` e recebe o QR code PIX gerado pelo Asaas (`POST /v3/pix/qrCodes/static`). A chave da API do Asaas fica **só no servidor** — nunca dentro do app.

## Rodar

```bash
export ASAAS_API_KEY='$aact_...'          # chave da API (Asaas > Integrações)
export ASAAS_PIX_KEY='sua-chave-pix'       # chave PIX cadastrada na conta Asaas
export ASAAS_ENV=production                # ou sandbox para testar
node server.mjs                            # Node 20+, sem dependências
```

Teste: `curl https://doacao.seudominio.com/pix?value=10`.

## Publicar com HTTPS

O app só aceita HTTPS. Exemplo com Caddy (certificado automático):

```
doacao.seudominio.com {
    reverse_proxy 127.0.0.1:8787
}
```

Para manter rodando, use `systemd` ou `pm2 start server.mjs --name commito-donation`.

## Apontar o app para a API

Defina a URL **no momento do build** do Commito:

```powershell
$env:COMMITO_DONATION_API_URL = "https://doacao.seudominio.com/pix"
pnpm tauri build
```

Sem essa variável, o botão de doação mostra que as doações ainda não estão configuradas. Para testar sem rebuild, a mesma variável de ambiente também é lida quando o app inicia.

## Notas

- QR codes estáticos aceitam vários pagamentos; o serviço guarda em cache um QR por valor por 6 horas para poupar a cota do Asaas.
- Há limite de 30 requisições por minuto por IP.
- Valores aceitos: de R$ 1 a R$ 10.000; sem `value`, o pagador escolhe o valor.
