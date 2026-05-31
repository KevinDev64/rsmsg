# rsmsg Developer Guide

This document describes the project structure, local development setup, checks, and core development rules.

## Workspace Structure

The project is a Rust workspace.

Main crates:

- `crates/shared`: DTOs and protocol types shared by client and server.
- `crates/crypto`: crypto helpers and usage of cryptographic primitives.
- `crates/client-core`: client transport, sessions, local storage, and E2EE client logic.
- `crates/client-ui`: desktop UI on `egui/eframe`, calls, tray, notifications, and localization.
- `crates/server`: HTTP/WebSocket API on `axum`, PostgreSQL repository layer, and invite-only auth.

Additional directories:

- `deploy`: local PostgreSQL infrastructure for development.
- `deploy/prod`: production Docker Compose, nginx example, and service scripts.
- `scripts/release`: release packaging scripts.
- `docs`: documentation.

## Requirements

- Rust stable.
- Docker and Docker Compose.
- PostgreSQL through Docker Compose.
- `sqlx-cli` for migrations.
- CMake for bundled Opus builds.

Install `sqlx-cli`:

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

The production server uses `~/.cargo/bin/sqlx`.

## Local Database

Copy the env file:

```bash
cp deploy/.env.example deploy/.env
```

Start PostgreSQL:

```bash
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d
```

Run migrations:

```bash
export DATABASE_URL=postgres://rsmsg:rsmsg_dev_password@127.0.0.1:5432/rsmsg
sqlx migrate run --source crates/server/migrations
```

## Local Run

Run the server:

```bash
cargo run -p server
```

The server listens on `127.0.0.1:3000` by default.

Run the client:

```bash
cargo run -p client-ui
```

For two clients on the same machine, use separate profiles:

```bash
RSMSG_PROFILE=alice cargo run -p client-ui
RSMSG_PROFILE=bob cargo run -p client-ui
```

## Invite Codes

Registration is closed without a one-time invite code.

Generate an invite locally:

```bash
cargo run -p server --bin generate_invite
```

Code format:

```text
RSMSG:<uuid>:<secret>
```

The code is valid for 2 days and becomes invalid after successful registration.

## Checks Before Changes

Minimum check set:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

If shell scripts changed:

```bash
bash -n scripts/release/generate-manifest.sh
bash -n scripts/release/build-linux.sh
bash -n scripts/release/build-macos.sh
```

If only one crate changed, you can check it first:

```bash
cargo check -p server
cargo check -p client-ui
```

Before merge or release, run workspace checks.

## Localization

Client localization is stored in JSON files:

```text
crates/client-ui/locales/en.json
crates/client-ui/locales/ru.json
```

Server API errors stay in English. The client localizes known strings for users.

When adding UI text, add keys at least to `en.json` and `ru.json`.

## Messages And E2EE

Messages and files are encrypted on the client.

Important properties:

- the server does not see plaintext messages;
- the server does not see plaintext files;
- the server stores encrypted envelopes and encrypted blobs;
- peer sessions are stored locally on the client;
- key changes should show a warning in the client.

Do not write custom cryptography unless there is a concrete need. Use existing primitives and reviewed libraries.

## Calls

Calls have two layers:

- call signaling through server API;
- WebRTC media transport for audio and video.

Signaling is stored in server process memory. The current version is designed for one backend instance. Multi-instance production requires shared state or a message broker for presence and call signaling.

Audio and video are encrypted with standard WebRTC DTLS-SRTP. This is separate from the application-level E2EE layer used for messages and files.

## Production Deploy

Production assets are in `deploy/prod`.

Main commands:

```bash
deploy/prod/update-server.sh
deploy/prod/update-with-turn.sh
deploy/prod/generate-invite.sh
deploy/prod/logs.sh
```

Details: `deploy/prod/README.md`.

## Release

The release workflow is described in `docs/release.md`.

Short version:

1. Work in `dev`.
2. For release, move `dev` into `master`.
3. Bump workspace version.
4. Create tag `vX.Y.Z` on `master`.
5. GitHub Actions builds release assets.
6. Generate the manifest and publish only `stable/manifest.json`.

Generate manifest:

```bash
VERSION=X.Y.Z GITHUB_REPO=KevinDev64/rsmsg scripts/release/generate-manifest.sh
```
