#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
ENV_FILE="$SCRIPT_DIR/.env"

echo "rsmsg turn update started"

if [ ! -f "$ENV_FILE" ]; then
  echo "missing env file: $ENV_FILE"
  echo "create it from $SCRIPT_DIR/.env.example"
  exit 1
fi

echo "building and restarting server stack with coturn"
"$SCRIPT_DIR/update-server.sh"
docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" --profile turn up -d coturn

echo "rsmsg turn update finished"
