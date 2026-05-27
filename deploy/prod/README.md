# rsmsg production deploy assets

TURN is required for reliable calls when peers are behind NAT or restrictive firewalls. STUN only tells clients their public address; TURN relays media when direct peer-to-peer ICE candidates cannot connect.

Files:
- `docker-compose.yml`: server, Postgres, optional Coturn profile.
- `.env.example`: production environment template.
- `nginx.conf.example`: reverse proxy location/server template for `rsmsg.kevindev64.ru` and certbot-managed TLS.
- Default server host port: `127.0.0.1:4222`.

Useful commands:
- Build/start server and Postgres: `docker compose -f deploy/prod/docker-compose.yml --env-file deploy/prod/.env up -d --build`
- Start optional TURN: `docker compose -f deploy/prod/docker-compose.yml --env-file deploy/prod/.env --profile turn up -d coturn`
- Run migrations from host: `DATABASE_URL=postgres://rsmsg:<password>@127.0.0.1:5442/rsmsg sqlx migrate run --source crates/server/migrations`
- Generate invite in container: `docker compose -f deploy/prod/docker-compose.yml --env-file deploy/prod/.env exec server /app/generate_invite`

Client server URL:
- `https://rsmsg.kevindev64.ru`

Client ICE settings when TURN is enabled:
- `stun:rsmsg.kevindev64.ru:3478`
- `turn:rsmsg.kevindev64.ru:3478`
