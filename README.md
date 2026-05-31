# rsmsg

rsmsg is a Rust desktop messenger with encrypted messages, encrypted file transfer, invite-only registration, blocking, localization, tray integration, notifications, and WebRTC audio/video calls.

The project contains both the desktop client and the backend server.

## Features

- Desktop client built with `egui/eframe`.
- Backend server built with `axum`, `tokio`, `sqlx`, and PostgreSQL.
- Invite-only registration.
- End-to-end encrypted messages and files at application level.
- Encrypted local client vault for keys, sessions, and history.
- File transfer up to `100 MB`.
- User blocking with server-side enforcement.
- Online presence.
- Audio and video calls through WebRTC.
- WebRTC media encryption through DTLS-SRTP.
- Client localization through external JSON files.
- Cross-platform packaging scripts for Windows, macOS, and Linux.
- GitHub Release based update assets and manifest-based update checks.

## Documentation

- User guide: `docs/USER_GUIDE.md`
- Developer guide: `docs/DEVELOPER_GUIDE.md`
- Contribution guide: `docs/CONTRIBUTING.md`
- Architecture overview: `docs/ARCHITECTURE.md`
- User guide RU: `docs/USER_GUIDE_RU.md`
- Developer guide RU: `docs/DEVELOPER_GUIDE_RU.md`
- Contribution guide RU: `docs/CONTRIBUTING_RU.md`
- Architecture overview RU: `docs/ARCHITECTURE_RU.md`
- Release process: `docs/release.md`
- Local startup guide in Russian: `starting.md`
- Local database guide: `deploy/README.md`
- Production deploy guide: `deploy/prod/README.md`
- API notes: `docs/RUST_API.md`

## Quick Start For Users

Download the latest release from GitHub Releases.

For the private production server, use this server URL in the client:

```text
https://rsmsg.kevindev64.ru
```

Registration requires a one-time invite code from the server administrator.

Full user documentation: `docs/USER_GUIDE.md`.

## Quick Start For Developers

Install Rust stable, Docker, Docker Compose, and `sqlx-cli`.

Start local PostgreSQL:

```bash
cp deploy/.env.example deploy/.env
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d
```

Run migrations:

```bash
export DATABASE_URL=postgres://rsmsg:rsmsg_dev_password@127.0.0.1:5432/rsmsg
sqlx migrate run --source crates/server/migrations
```

Run server:

```bash
cargo run -p server
```

Run client:

```bash
cargo run -p client-ui
```

Run checks:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

Full developer documentation: `docs/DEVELOPER_GUIDE.md`.

## Workspace Layout

```text
crates/shared       shared DTOs and protocol types
crates/crypto       crypto helpers and primitives usage
crates/client-core  client transport, E2EE sessions, local storage
crates/client-ui    desktop UI, calls, notifications, tray
crates/server       backend API, auth, messaging, blobs, signaling
deploy              local development database
deploy/prod         production Docker/nginx/coturn assets
scripts/release     release packaging and manifest scripts
docs                project documentation
```

## Security Model

Messages and files are encrypted on the client before upload. The server stores encrypted envelopes and encrypted blobs.

Audio and video calls use WebRTC media encryption through DTLS-SRTP. This protects the media transport, including TURN relay paths, but it is separate from the application-level E2EE used for stored messages and files.

Do not treat this README as a full cryptographic audit. See `docs/ARCHITECTURE.md` and the code for implementation details.

## Branches

- `dev`: active development.
- `master`: releases only.

Contributions should target `dev` through pull requests. See `docs/CONTRIBUTING.md`.

## Release

Release assets are built by GitHub Actions from tags on `master`.

The update manifest is generated locally with:

```bash
VERSION=X.Y.Z GITHUB_REPO=KevinDev64/rsmsg scripts/release/generate-manifest.sh
```

Only `rsmsg-downloads/stable/manifest.json` is hosted on `kevindev64.ru`.

## License

MIT
