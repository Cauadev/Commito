#!/usr/bin/env bash
# Idempotent installer for the Commito server pieces. Run as root on the VPS from the
# directory that contains `deploy/` and `donation-api/` (uploaded by deploy-vps.sh).
set -euo pipefail
SRC=$(cd "$(dirname "$0")/.." && pwd)

id commito >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin commito

install -d -m 755 /opt/commito/donation-api /opt/commito/deploy
install -m 644 "$SRC/donation-api/server.mjs" /opt/commito/donation-api/server.mjs
install -m 755 "$SRC/deploy/commito-cert.sh" /opt/commito/deploy/commito-cert.sh

# Secrets file: created once with empty values; never overwritten.
install -d -m 750 /etc/commito
if [ ! -f /etc/commito/donation.env ]; then
  cat > /etc/commito/donation.env <<'ENV'
# Preencha e depois rode: systemctl restart commito-donation
ASAAS_API_KEY=
ASAAS_PIX_KEY=
ASAAS_ENV=production
DESCRIPTION=Doação Commito
ENV
fi
chown root:commito /etc/commito /etc/commito/donation.env
chmod 640 /etc/commito/donation.env

install -d -m 755 /var/www/commito/releases

install -m 644 "$SRC/deploy/commito-donation.service" /etc/systemd/system/commito-donation.service
install -m 644 "$SRC/deploy/commito-cert.service" /etc/systemd/system/commito-cert.service
install -m 644 "$SRC/deploy/commito-cert.timer" /etc/systemd/system/commito-cert.timer

# nginx: a separate site file; existing sites are untouched. Keep certbot's edits on re-deploy.
SITE=/etc/nginx/sites-available/commito.cauadev.com
if [ ! -f "$SITE" ] || ! grep -q "managed by Certbot" "$SITE"; then
  install -m 644 "$SRC/deploy/commito.cauadev.com.nginx" "$SITE"
fi
ln -sf "$SITE" /etc/nginx/sites-enabled/commito.cauadev.com
nginx -t
systemctl reload nginx

systemctl daemon-reload
systemctl enable --now commito-donation.service
systemctl restart commito-donation.service
if [ ! -d /etc/letsencrypt/live/commito.cauadev.com ]; then
  systemctl enable --now commito-cert.timer
fi
echo "OK"
