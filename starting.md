# Запуск и проверка rsmsg

## 1) Требования

- Rust (stable) и Cargo
- Docker + Docker Compose
- `sqlx-cli`

Установка `sqlx-cli`:

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

## 2) Поднять PostgreSQL

Скопировать env-файл:

```bash
cp deploy/.env.example deploy/.env
```

Запустить БД:

```bash
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d
```

## 3) Применить миграции

Экспортировать строку подключения:

```bash
export DATABASE_URL=postgres://rsmsg:rsmsg_dev_password@127.0.0.1:5432/rsmsg
```

Применить миграции:

```bash
sqlx migrate run --source crates/server/migrations
```

## 4) Проверить сборку

```bash
cargo fmt --all
cargo check --workspace
cargo test -p server
```

## 5) Сгенерировать invite-коды

Регистрация закрыта для всех пользователей без одноразового invite-кода.
Код живёт 2 дня и сгорает после успешной регистрации.

```bash
cargo run -p server --bin generate_invite
```

Формат кода: `RSMSG:<uuid>:<secret>`.

## 6) Запустить сервер

```bash
cargo run -p server
```

По умолчанию сервер слушает `127.0.0.1:3000`.

## 7) Запустить клиент (egui)

В новом терминале из корня проекта:

```bash
cargo run -p client-ui
```

Для запуска нескольких клиентов на одной машине используйте разные профили,
иначе они будут делить одни и те же локальные ключи/сессии/историю:

```bash
RSMSG_PROFILE=alice cargo run -p client-ui
RSMSG_PROFILE=bob cargo run -p client-ui
```

## 8) Сценарий ручного теста (2 пользователя)

Откройте **два** окна клиента (`client-ui`).

### Окно A

1. Введите:
   - `Nickname`: `alice`
   - `Password`: любой пароль длиной 6+ символов
2. Нажмите `Create account`.
3. Введите nickname, password и invite code.
4. Нажмите `Create account`.

### Окно B

1. Введите:
   - `Nickname`: `bob`
   - `Password`: любой пароль длиной 6+ символов
2. Нажмите `Create account`.
3. Введите nickname, password и invite code.
4. Нажмите `Create account`.

### Создание чатов

1. В A: в `Peer nickname` введите `bob`.
2. Нажмите `Search users` (опционально) и выберите `@bob`.
3. Нажмите `Open chat`.
4. В B аналогично откройте чат с `alice`.

### Обмен сообщениями

1. Введите текст и нажмите `Send`.
2. Входящие подтягиваются автоматически (периодический sync).
3. История чатов отображается в центре и сохраняется локально.

## 9) Полезные API endpoint'ы (server)

- `POST /v1/user_register`
- `POST /v1/user_login`
- `POST /v1/user_search`
- `POST /v1/register_device`
- `POST /v1/device_login`
- `POST /v1/resolve_user`
- `POST /v1/fetch_prekey_bundle`
- `POST /v1/send_message`
- `POST /v1/fetch_pending`
- `POST /v1/ack_message`
- `GET /v1/ws`

## 10) Локальные файлы состояния клиента

Файлы создаются в корне проекта автоматически:

- `.rsmsg_local_keys.json`
- `.rsmsg_peer_sessions.json`
- `.rsmsg_chat_history.json`

Они добавлены в `.gitignore`.

## 11) Локализация клиента

UI-тексты клиента лежат в JSON-файлах:

```text
crates/client-ui/locales/en.json
crates/client-ui/locales/ru.json
```

Локализаторам не нужно менять Rust-код: достаточно добавить или отредактировать ключи в JSON-файлах.
Если ключ отсутствует в выбранном языке, клиент использует английский fallback.

## 12) Остановка

Остановить сервер: `Ctrl+C` в терминале сервера.

Остановить БД:

```bash
docker compose -f deploy/docker-compose.yml --env-file deploy/.env down
```
