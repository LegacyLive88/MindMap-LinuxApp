#!/usr/bin/env bash
# Restart the cloud if it is not answering. Useful as a cPanel cron every few minutes
# when the account is a jailed shell and the Node process does not stay up on its own.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi
port="${PORT:-3000}"
if curl -fsS -o /dev/null "http://127.0.0.1:${port}/api/v1/health"; then
  exit 0
fi
mkdir -p "$HOME/logs"
nohup npm start >> "$HOME/logs/mindmap-cloud.log" 2>&1 &
echo "Started MindMap cloud on 127.0.0.1:${port}"
