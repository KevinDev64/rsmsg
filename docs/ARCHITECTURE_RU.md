# Архитектура rsmsg

Этот документ даёт краткую техническую картину проекта.

## Компоненты

rsmsg состоит из desktop-клиента, backend-сервера и PostgreSQL.

Клиент:

- показывает UI;
- хранит локальные ключи и историю;
- шифрует сообщения и файлы;
- расшифровывает входящие сообщения;
- выполняет sync pending messages;
- управляет WebRTC-звонками.

Сервер:

- обрабатывает регистрацию и вход;
- проверяет invite-коды;
- хранит device records и public prekey bundles;
- хранит encrypted message envelopes;
- хранит encrypted blobs;
- проверяет block rules;
- отдаёт pending messages;
- принимает read ack;
- хранит presence и call signaling в памяти процесса.

PostgreSQL:

- хранит пользователей;
- хранит devices;
- хранит auth tokens;
- хранит invites;
- хранит encrypted messages;
- хранит encrypted blobs;
- хранит block relationships.

## Основной поток регистрации

1. Администратор генерирует invite-код.
2. Пользователь вводит никнейм, пароль и invite-код.
3. Сервер проверяет invite-код.
4. Сервер сохраняет Argon2 password hash.
5. Invite-код помечается использованным.
6. Клиент создаёт локальные ключи устройства.
7. Клиент регистрирует device и public key material на сервере.
8. Клиент логинится как device и получает auth token.

## Основной поток сообщений

1. Отправитель открывает чат с получателем.
2. Клиент отправителя получает public bundle получателя.
3. Клиент отправителя создаёт peer session.
4. Сообщение шифруется локально.
5. Сервер получает encrypted envelope.
6. Получатель периодически вызывает `fetch_pending`.
7. Сервер отдаёт pending encrypted envelopes.
8. Клиент получателя расшифровывает сообщения локально.
9. После чтения клиент отправляет `ack_message`.
10. Сервер помечает сообщение прочитанным.

До read ack сообщение остаётся pending и может быть выдано повторно. Клиент дедуплицирует входящие сообщения по `message_id`.

## Файлы

Файлы шифруются клиентом и загружаются как encrypted blobs.

Ограничение файла: `100 MB`.

Сервер не должен видеть:

- plaintext файла;
- plaintext имени файла;
- file key.

## Звонки

Звонки используют call signaling через сервер и WebRTC для media transport.

Сигналинг включает:

- `invite`;
- `answer`;
- `decline`;
- `busy`;
- `hangup`;
- `webrtc-offer`;
- `webrtc-answer`.

Presence и call signaling сейчас in-memory. Это означает, что production должен использовать один backend instance. Для нескольких instances понадобится общий state layer.

Аудио и видео шифруются WebRTC DTLS-SRTP. TURN relays видят зашифрованные SRTP-пакеты.

## Version gate

Клиент отправляет version headers:

- `x-rsmsg-client-version`;
- `x-rsmsg-platform`;
- `x-rsmsg-protocol-version`.

Сервер может отклонить устаревший клиент через `426 Upgrade Required`, если `MIN_CLIENT_VERSION` выше версии клиента.

## Обновления

Клиент проверяет update manifest:

```text
https://kevindev64.ru/rsmsg-downloads/stable/manifest.json
```

Manifest содержит ссылки на GitHub Release assets.

## Ограничения текущей архитектуры

- Presence in-memory и не multi-instance safe.
- Call signaling in-memory и не multi-instance safe.
- Message ratchet не является полной реализацией Signal Double Ratchet.
- Нет восстановления пароля.
- Нет server-side удаления сообщений у собеседника.
- Local delete удаляет только локальную историю.
