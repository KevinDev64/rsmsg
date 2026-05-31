# Руководство разработчика rsmsg

Этот документ описывает структуру проекта, локальный запуск, проверки и базовые правила разработки.

## Структура workspace

Проект — Rust workspace.

Основные crates:

- `crates/shared`: DTO и типы, общие для клиента и сервера.
- `crates/crypto`: криптографические операции и helpers.
- `crates/client-core`: транспорт, сессии, локальное хранилище, E2EE-логика клиента.
- `crates/client-ui`: desktop UI на `egui/eframe`, звонки, tray, локализация.
- `crates/server`: HTTP/WebSocket API на `axum`, Postgres repository layer, invite-only auth.

Дополнительные директории:

- `deploy`: локальная PostgreSQL-инфраструктура для разработки.
- `deploy/prod`: production Docker Compose, nginx example и service scripts.
- `scripts/release`: release packaging scripts.
- `docs`: документация.

## Требования

- Rust stable.
- Docker и Docker Compose.
- PostgreSQL через Docker Compose.
- `sqlx-cli` для миграций.
- CMake для сборки bundled Opus.

Установка `sqlx-cli`:

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

На production-сервере используется `~/.cargo/bin/sqlx`.

## Локальная база данных

Скопируйте env-файл:

```bash
cp deploy/.env.example deploy/.env
```

Запустите PostgreSQL:

```bash
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d
```

Примените миграции:

```bash
export DATABASE_URL=postgres://rsmsg:rsmsg_dev_password@127.0.0.1:5432/rsmsg
sqlx migrate run --source crates/server/migrations
```

## Локальный запуск

Запуск сервера:

```bash
cargo run -p server
```

Сервер по умолчанию слушает `127.0.0.1:3000`.

Запуск клиента:

```bash
cargo run -p client-ui
```

Для двух клиентов на одной машине используйте разные профили:

```bash
RSMSG_PROFILE=alice cargo run -p client-ui
RSMSG_PROFILE=bob cargo run -p client-ui
```

## Invite-коды

Регистрация закрыта без одноразового invite-кода.

Сгенерировать invite локально:

```bash
cargo run -p server --bin generate_invite
```

Формат кода:

```text
RSMSG:<uuid>:<secret>
```

Код живёт 2 дня и становится недействительным после регистрации.

## Проверки перед изменениями

Минимальный набор проверок:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

Если менялись shell scripts:

```bash
bash -n scripts/release/generate-manifest.sh
bash -n scripts/release/build-linux.sh
bash -n scripts/release/build-macos.sh
```

Если менялся только один crate, можно сначала проверить его:

```bash
cargo check -p server
cargo check -p client-ui
```

Перед merge/release всё равно запускайте workspace checks.

## Локализация

Клиентская локализация находится в JSON-файлах:

```text
crates/client-ui/locales/en.json
crates/client-ui/locales/ru.json
```

Серверные API errors остаются на английском. Клиент локализует известные строки для пользователя.

При добавлении UI-текста добавляйте ключи минимум в `en.json` и `ru.json`.

## Сообщения и E2EE

Сообщения и файлы шифруются на клиенте.

Важные свойства:

- сервер не видит plaintext сообщений;
- сервер не видит plaintext файлов;
- сервер хранит encrypted envelopes и encrypted blobs;
- peer sessions хранятся локально на клиенте;
- при смене ключа клиент должен показать предупреждение.

Не пишите собственную криптографию без необходимости. Используйте существующие primitives и проверенные библиотеки.

## Звонки

Звонки состоят из двух слоёв:

- call signaling через server API;
- WebRTC media transport для аудио и видео.

Сигналинг хранится в памяти server process. Текущая версия рассчитана на один backend instance. Для multi-instance production потребуется внешнее shared-хранилище или message broker для presence и call signaling.

Аудио и видео шифруются стандартным WebRTC DTLS-SRTP. Это не тот же application-level E2EE слой, который используется для сообщений и файлов.

## Production deploy

Production assets находятся в `deploy/prod`.

Основные команды:

```bash
deploy/prod/update-server.sh
deploy/prod/update-with-turn.sh
deploy/prod/generate-invite.sh
deploy/prod/logs.sh
```

Подробности: `deploy/prod/README.md`.

## Release

Release workflow описан в `docs/release.md`.
