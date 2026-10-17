#!/usr/bin/env bash
# Issues the HTTPS certificate for commito.cauadev.com as soon as its DNS points to this server.
# Run by commito-cert.timer; disables the timer after success (renewal is handled by certbot's own timer).
set -euo pipefail
DOMAIN=commito.cauadev.com
SERVER_IP=187.77.246.113

if [ -d "/etc/letsencrypt/live/$DOMAIN" ]; then
  systemctl disable --now commito-cert.timer || true
  exit 0
fi

RESOLVED=$(getent ahostsv4 "$DOMAIN" | awk '{print $1; exit}' || true)
if [ "$RESOLVED" != "$SERVER_IP" ]; then
  echo "DNS de $DOMAIN ainda não aponta para $SERVER_IP (atual: ${RESOLVED:-nenhum}); tentando novamente depois."
  exit 0
fi

# Uses the existing certbot account of this server; only the Commito site is modified.
certbot --nginx -d "$DOMAIN" --non-interactive --redirect --keep-until-expiring
systemctl disable --now commito-cert.timer || true
echo "Certificado emitido para $DOMAIN."
