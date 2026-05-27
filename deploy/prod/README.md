# rsmsg production deploy assets

TURN is required for reliable calls when peers are behind NAT or restrictive firewalls. STUN only tells clients their public address; TURN relays media when direct peer-to-peer ICE candidates cannot connect.

Files:
- `docker-compose.yml`: server, Postgres, optional Coturn profile.
- `.env.example`: production environment template.
- `nginx.conf.example`: reverse proxy location/server template for `rsmsg.kevindev64.ru` and certbot-managed TLS.
- `update-server.sh`: starts Postgres, runs migrations, rebuilds/restarts server, checks health.
- `update-with-turn.sh`: runs server update and starts Coturn profile.
- `generate-invite.sh`: generates an invite through the server container.
- `logs.sh`: follows logs for `server` by default, or for the service passed as the first argument.
- Default server host port: `127.0.0.1:4222`.

Useful commands:
- Update server: `deploy/prod/update-server.sh`
- Update server and TURN: `deploy/prod/update-with-turn.sh`
- Generate invite: `deploy/prod/generate-invite.sh`
- Follow server logs: `deploy/prod/logs.sh`
- Follow Postgres logs: `deploy/prod/logs.sh postgres`
- Follow TURN logs: `deploy/prod/logs.sh coturn`
- Build/start server and Postgres: `docker compose -f deploy/prod/docker-compose.yml --env-file deploy/prod/.env up -d --build`
- Start optional TURN: `docker compose -f deploy/prod/docker-compose.yml --env-file deploy/prod/.env --profile turn up -d coturn`
- Run migrations from host: `DATABASE_URL=postgres://rsmsg:<password>@127.0.0.1:5442/rsmsg sqlx migrate run --source crates/server/migrations`
- Generate invite in container: `docker compose -f deploy/prod/docker-compose.yml --env-file deploy/prod/.env exec server /app/generate_invite`

Client server URL:
- `https://rsmsg.kevindev64.ru`

Client ICE settings when TURN is enabled:
- `stun:rsmsg.kevindev64.ru:3478`
- `turn:rsmsg.kevindev64.ru:3478`
