# rsmsg Architecture

This document gives a short technical overview of the project.

## Components

rsmsg consists of a desktop client, backend server, and PostgreSQL database.

Client:

- shows the UI;
- stores local keys and history;
- encrypts messages and files;
- decrypts incoming messages;
- syncs pending messages;
- manages WebRTC calls.

Server:

- handles registration and login;
- validates invite codes;
- stores device records and public prekey bundles;
- stores encrypted message envelopes;
- stores encrypted blobs;
- enforces block rules;
- returns pending messages;
- accepts read acknowledgements;
- stores presence and call signaling in process memory.

PostgreSQL:

- stores users;
- stores devices;
- stores auth tokens;
- stores invites;
- stores encrypted messages;
- stores encrypted blobs;
- stores block relationships.

## Registration Flow

1. Administrator generates an invite code.
2. User enters nickname, password, and invite code.
3. Server validates the invite code.
4. Server stores the Argon2 password hash.
5. Invite code is marked as used.
6. Client creates local device keys.
7. Client registers the device and public key material on the server.
8. Client logs in as the device and receives an auth token.

## Message Flow

1. Sender opens a chat with the recipient.
2. Sender client fetches the recipient public bundle.
3. Sender client creates a peer session.
4. The message is encrypted locally.
5. Server receives an encrypted envelope.
6. Recipient periodically calls `fetch_pending`.
7. Server returns pending encrypted envelopes.
8. Recipient client decrypts messages locally.
9. After reading, the recipient client sends `ack_message`.
10. Server marks the message as read.

Until read ack, the message remains pending and can be returned again. The client deduplicates incoming messages by `message_id`.

## Files

Files are encrypted by the client and uploaded as encrypted blobs.

File limit: `100 MB`.

The server must not see:

- plaintext file content;
- plaintext file name;
- file key.

## Calls

Calls use signaling through the server and WebRTC for media transport.

Signal kinds:

- `invite`;
- `answer`;
- `decline`;
- `busy`;
- `hangup`;
- `webrtc-offer`;
- `webrtc-answer`.

Presence and call signaling are currently in-memory. This means production should use one backend instance. Multiple instances require a shared state layer.

Audio and video are encrypted with WebRTC DTLS-SRTP. TURN relays see encrypted SRTP packets.

## Version Gate

The client sends version headers:

- `x-rsmsg-client-version`;
- `x-rsmsg-platform`;
- `x-rsmsg-protocol-version`.

The server can reject an outdated client with `426 Upgrade Required` if `MIN_CLIENT_VERSION` is higher than the client version.

## Updates

The client checks the update manifest:

```text
https://kevindev64.ru/rsmsg-downloads/stable/manifest.json
```

The manifest contains links to GitHub Release assets. Installers are not stored on `kevindev64.ru`.

## Current Architecture Limits

- Presence is in-memory and not multi-instance safe.
- Call signaling is in-memory and not multi-instance safe.
- Message ratchet is not a full Signal Double Ratchet implementation.
- Password recovery is not implemented.
- Server-side deletion for the peer is not implemented.
- Local delete removes only local history.
