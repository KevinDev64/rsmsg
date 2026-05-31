# rsmsg User Guide

rsmsg is a desktop messenger with encrypted messages and encrypted file transfer. The app works through an rsmsg server, but the server does not receive plaintext messages, files, or file names.

## What You Need To Sign In

- Server address. For the main private server: `https://rsmsg.kevindev64.ru`.
- Nickname.
- Password with at least 6 characters.
- One-time invite code for new account registration.

The invite code is only required when creating an account. After registration, sign in with your nickname and password.

## First Launch

1. Open rsmsg.
2. Check the server field.
3. If the server is empty, enter `https://rsmsg.kevindev64.ru` or your own server address.
4. Enter your nickname and password.
5. If your account already exists, click `Login`.
6. If you do not have an account yet, click `Create account` and enter your invite code.

If the server requires a newer app version, rsmsg will show an update message.

## Creating An Account

1. Click `Create account`.
2. Enter your nickname.
3. Enter your password.
4. Enter your invite code.
5. Click the account creation button.

Invite codes are one-time use. If the code is already used or expired, ask the server administrator for a new code.

## Server Settings

You can change the server address in `Settings -> Connection`.

Regular users should enter the public server URL without the internal backend port. Example:

```text
https://rsmsg.kevindev64.ru
```

Do not use `https://rsmsg.kevindev64.ru:4222` if the server is behind nginx or another reverse proxy.

## Chats

To start a chat:

1. Sign in.
2. Enter the peer nickname in `Peer nickname`.
3. Click `Search users` if you want to check that the user exists.
4. Click `Open chat`.
5. After the chat opens, type a message and click `Send`.

If messages do not appear immediately, click `Sync incoming`. Usually sync runs automatically.

## Messages

Messages are encrypted on the sender device and decrypted on the recipient device. The server stores only encrypted data.

Message statuses:

- `Sending`: the message is being sent.
- `Sent`: the server accepted the message.
- `Delivered`: the recipient fetched the message from the server.
- `Read`: the recipient opened the chat and the message was marked as read.
- `Failed`: sending failed.

If the app says that pending messages were received but none were decrypted, re-open the chat with the peer. If the app shows a key-change warning, use `Trust new key` only if you are sure this is the same peer.

## Files

You can send files up to `100 MB`.

Files are encrypted on the sender device. The server does not see plaintext file content, file names, or file keys.

To send a file:

1. Open a chat.
2. Click `Attach file`.
3. Select a file.
4. Wait for upload to finish.

To save an incoming file, click `Save` next to the file message.

## Audio And Video Calls

You can call only users who are online.

Calls use WebRTC. Media is encrypted with standard WebRTC DTLS-SRTP. This protects audio and video in transit. It is separate from the application-level E2EE used by rsmsg for messages and files.

If TURN is used, the TURN server relays encrypted SRTP packets and should not see plaintext audio or video.

The call window supports:

- accept call;
- decline call;
- mute microphone;
- unmute microphone;
- turn camera off;
- turn camera on;
- hang up.

Hotkeys:

- `Esc`: hang up.
- `Ctrl+M` or `Command+M`: mute or unmute microphone.

## Call Settings

Select devices in `Settings -> Call devices`:

- microphone;
- speaker;
- camera.

Call network settings are in `Settings -> Call network`.

The default STUN server is:

```text
stun:stun.l.google.com:19302
```

For reliable calls through NAT or restrictive firewalls, TURN may be required. TURN credentials are provided by the server administrator.

## Blocking Users

You can block a user in a chat or in `Settings -> Privacy`.

After blocking:

- the blocked user cannot send you messages;
- you cannot send messages to the blocked user;
- calls between you are blocked.

You can unblock users in `Settings -> Privacy`.

## Local Data

rsmsg stores local keys, sessions, and history on your device.

Default directories:

- macOS: `~/Library/Application Support/rsmsg`
- Windows: `%APPDATA%\rsmsg`
- Linux: `$XDG_DATA_HOME/rsmsg` or `~/.local/share/rsmsg`

For testing multiple accounts on one machine, use separate profiles:

```bash
RSMSG_PROFILE=alice rsmsg
RSMSG_PROFILE=bob rsmsg
```

If you delete local data, the app loses local history, device keys, and saved sessions.

## Updates

The app checks the update manifest at:

```text
https://kevindev64.ru/rsmsg-downloads/stable/manifest.json
```

If an update is mandatory, login may be blocked until you install the newer version.

## Common Problems

### Invalid Login Or Password

Check your nickname and password. Password recovery is not implemented in the current version.

### Server Unavailable

Check the server address in `Settings -> Connection`. For the private server, use `https://rsmsg.kevindev64.ru`.

### User Is Offline

Calls are possible only when the recipient is online. Ask the recipient to open the app and sign in.

### Messages Cannot Be Decrypted

Re-open the chat with the peer. If a key-change warning appears, trust the new key only after confirming that it is really the peer's device.

### Call Does Not Connect

Check microphone, camera, and network settings. If both users are behind NAT or firewalls, a TURN server is required.
