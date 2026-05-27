#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
ENV_FILE="$SCRIPT_DIR/.env"

echo "rsmsg server update started"

if [ ! -f "$ENV_FILE" ]; then
  echo "missing env file: $ENV_FILE"
  echo "create it from $SCRIPT_DIR/.env.example"
  exit 1
fi

set -a
source "$ENV_FILE"
set +a

POSTGRES_PORT="${POSTGRES_PORT:-5442}"
RSMSG_HTTP_PORT="${RSMSG_HTTP_PORT:-4222}"
DATABASE_URL="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@127.0.0.1:${POSTGRES_PORT}/${POSTGRES_DB}"

echo "using compose file: $COMPOSE_FILE"
echo "using env file: $ENV_FILE"
echo "postgres host port: $POSTGRES_PORT"
echo "server host port: $RSMSG_HTTP_PORT"

echo "starting postgres"
docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" up -d postgres

echo "waiting for postgres"
docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" exec -T postgres pg_isready -U "$POSTGRES_USER" -d "$POSTGRES_DB"

echo "running migrations"
if ! command -v sqlx >/dev/null 2>&1; then
  echo "sqlx command not found on host"
  echo "install sqlx-cli or run migrations manually"
  exit 1
fi
DATABASE_URL="$DATABASE_URL" sqlx migrate run --source "$ROOT_DIR/crates/server/migrations"

echo "building and restarting server"
docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" up -d --build server

echo "checking local health"
curl -fsS "http://127.0.0.1:${RSMSG_HTTP_PORT}/health"
echo

echo "rsmsg server update finished"
