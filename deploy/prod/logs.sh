#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
ENV_FILE="$SCRIPT_DIR/.env"
SERVICE="${1:-server}"

echo "showing logs for $SERVICE"

if [ ! -f "$ENV_FILE" ]; then
  echo "missing env file: $ENV_FILE"
  echo "create it from $SCRIPT_DIR/.env.example"
  exit 1
fi

docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" logs -f --tail=200 "$SERVICE"
