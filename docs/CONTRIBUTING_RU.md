# Как вносить изменения

Этот документ описывает рабочий процесс для разработчиков, которые хотят внести изменения в rsmsg.

## Ветки

- `dev`: основная ветка разработки.
- `master`: только релизы.

Не отправляйте обычные изменения напрямую в `master`. Все feature/fix изменения должны идти через ветку от `dev` и pull request обратно в `dev`.

## Создание ветки

Обновите `dev`:

```bash
git checkout dev
git pull origin dev
```

Создайте рабочую ветку:

```bash
git checkout -b fix/short-description
```

Примеры имён веток:

```text
fix/login-timeout
feat/file-preview
docs/user-guide
ci/release-manifest
```

## Перед началом работы

Проверьте, что проект собирается:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

Если вы меняете только документацию, Rust-проверки обычно не обязательны, но `git diff` перед PR всё равно обязателен.

## Правила изменений

- Делайте минимальные корректные изменения.
- Не добавляйте compatibility layer без реальной необходимости.
- Не меняйте публичное поведение случайно.
- Не оставляйте secrets, tokens, private keys и реальные пароли в репозитории.
- Не пишите свою криптографию с нуля.
- Пользовательские строки клиента должны идти через JSON-локализацию.
- Не смешивайте изменения разного рода в одном PR.

## Коммиты

Формат commit message:

```text
type: message
```

Разрешённые типы:

```text
feat
fix
chore
hotfix
ci
docs
style
refac
```

Примеры:

```text
fix: handle stale message sessions
feat: add blocked users refresh
docs: add user guide
ci: publish github release assets
```

## Проверки перед Pull Request

Перед открытием PR выполните:

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

Если менялся release workflow, проверьте `.github/workflows/release.yml` и убедитесь, что tag guard на `master` сохранён.

## Pull Request

PR должен идти в `dev`.

В описании PR укажите:

- что изменилось;
- зачем это нужно;
- какие проверки запускались;
- есть ли миграции БД;
- есть ли изменения формата локальных данных;
- есть ли изменения в протоколе client/server;
- нужен ли bump версии клиента или server minimum version.

Шаблон описания:

```markdown
## Summary

- 

## Testing

- [ ] cargo fmt --all --check
- [ ] cargo check --workspace
- [ ] cargo test --workspace

## Notes

- Database migration: no
- Protocol change: no
- Local data format change: no
```

## Code review

Перед merge проверьте:

- нет ли plaintext утечек сообщений, файлов или ключей;
- не ломается ли invite-only регистрация;
- не ломается ли block logic;
- не ломается ли delivery retry до read ack;
- не ломается ли backward behavior для уже существующих пользователей;
- есть ли понятный пользовательский status вместо сырой технической ошибки;
- обновлена ли документация, если изменился пользовательский сценарий.

## Миграции БД

Миграции лежат в:

```text
crates/server/migrations
```

После добавления миграции проверьте локальный запуск:

```bash
export DATABASE_URL=postgres://rsmsg:rsmsg_dev_password@127.0.0.1:5432/rsmsg
sqlx migrate run --source crates/server/migrations
cargo test -p server
```

## Изменения API

Если меняются DTO в `crates/shared`, проверьте обе стороны:

```bash
cargo check -p server
cargo check -p client-ui
```

Если старый клиент больше не совместим, обновите release notes и настройку `MIN_CLIENT_VERSION` для production deploy.

## Security-sensitive изменения

Для изменений в E2EE, local vault, key storage, session repair, auth tokens или invite codes нужна отдельная внимательная проверка.

Минимально проверьте:

- регистрация нового пользователя;
- вход существующего пользователя;
- создание чата;
- первое сообщение в обе стороны;
- повторный вход после перезапуска клиента;
- смена ключа и `Trust new key`;
- отправка файла;
- block/unblock;
- звонок.

## Release changes

Release делается из `master`.

Не создавайте tag без явного решения о release version.

Для patch release используйте следующий свободный patch tag, например `v1.0.2`, `v1.0.3` и так далее.
