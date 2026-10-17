# Servidor do Commito (`commito.cauadev.com`)

| Rota | O que é |
|---|---|
| `GET /api/pix?value=10` | QR code PIX de doação (gerado pelo Asaas) usado pelo app |
| `GET /api/health` | `{ ok, configured }` |
| `GET /releases/latest.json` | Feed do auto-update (sem cache) |
| `GET /releases/Commito-setup.exe` | Sempre o instalador mais recente |
| `GET /releases/Commito_<versão>_x64-setup.exe` (+ `.sig`) | Instaladores versionados e assinaturas |

## O que está na VPS

- `commito-donation.service`: Node (`/opt/commito/donation-api/server.mjs`) escutando só em `127.0.0.1:8787`, usuário de sistema `commito`, com hardening do systemd.
- `/etc/commito/donation.env` (modo 640): **as chaves do Asaas ficam só aqui**.
- nginx: site próprio `/etc/nginx/sites-available/commito.cauadev.com` (os outros sites não são alterados), com limite de 20 req/min por IP no `/api/pix`.
- `/var/www/commito/releases`: arquivos publicados por `pnpm release -- --publish-vps`.
- `commito-cert.timer`: a cada 10 min verifica se o DNS de `commito.cauadev.com` já aponta para a VPS e emite o HTTPS (certbot); depois se desativa. A renovação é a do certbot.

## Configurar o Asaas (uma vez)

```bash
ssh root@187.77.246.113
nano /etc/commito/donation.env        # preencha ASAAS_API_KEY e ASAAS_PIX_KEY
systemctl restart commito-donation
curl -s https://commito.cauadev.com/api/health   # "configured": true
```

## Reinstalar / atualizar o servidor

```bash
scp -r server/deploy server/donation-api root@187.77.246.113:/tmp/commito-deploy/
ssh root@187.77.246.113 'bash /tmp/commito-deploy/deploy/install.sh'
```

O instalador é idempotente: não sobrescreve `donation.env` nem as alterações de HTTPS feitas pelo certbot.
