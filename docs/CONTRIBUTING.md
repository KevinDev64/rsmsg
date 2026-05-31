# Contributing To rsmsg

This document describes the workflow for developers who want to contribute changes to rsmsg.

## Branches

- `dev`: main development branch.
- `master`: releases only.

Do not send regular changes directly to `master`. Feature and fix changes should go through a branch from `dev` and a pull request back into `dev`.

## Create A Branch

Update `dev`:

```bash
git checkout dev
git pull origin dev
```

Create a working branch:

```bash
git checkout -b fix/short-description
```

Branch name examples:

```text
fix/login-timeout
feat/file-preview
docs/user-guide
ci/release-manifest
```

## Before Starting Work

Check that the project builds:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

If you are changing only documentation, Rust checks are usually not required, but reviewing `git diff` before PR is still required.

## Change Rules

- Make the smallest correct change.
- Do not add compatibility layers without a concrete need.
- Do not change public behavior accidentally.
- Do not commit secrets, tokens, private keys, or real passwords.
- Do not write custom cryptography from scratch.
- Server API errors must stay in English.
- User-visible client strings must go through JSON localization.
- Do not mix unrelated changes in one PR.

## Commits

Commit message format:

```text
type: message
```

Allowed types:

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

Examples:

```text
fix: handle stale message sessions
feat: add blocked users refresh
docs: add user guide
ci: publish github release assets
```

## Checks Before Pull Request

Before opening a PR, run:

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

If release workflow changed, review `.github/workflows/release.yml` and make sure the tag guard on `master` remains in place.

## Pull Request

PRs should target `dev`.

In the PR description, include:

- what changed;
- why it is needed;
- what checks were run;
- whether there are database migrations;
- whether local data format changed;
- whether client/server protocol changed;
- whether client version bump or server minimum version update is needed.

Description template:

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

## Code Review

Before merge, check:

- no plaintext leaks for messages, files, or keys;
- invite-only registration still works;
- block logic still works;
- delivery retry until read ack still works;
- existing user behavior is not broken;
- user-facing status is understandable and not a raw technical error;
- documentation is updated if a user flow changed.

## Database Migrations

Migrations are stored in:

```text
crates/server/migrations
```

After adding a migration, check local run:

```bash
export DATABASE_URL=postgres://rsmsg:rsmsg_dev_password@127.0.0.1:5432/rsmsg
sqlx migrate run --source crates/server/migrations
cargo test -p server
```

## API Changes

If DTOs in `crates/shared` change, check both sides:

```bash
cargo check -p server
cargo check -p client-ui
```

If the old client is no longer compatible, update release notes and the production `MIN_CLIENT_VERSION` setting.

## Security-Sensitive Changes

Changes in E2EE, local vault, key storage, session repair, auth tokens, or invite codes need extra review.

Minimum manual checks:

- new user registration;
- existing user login;
- chat creation;
- first message in both directions;
- login again after client restart;
- key change and `Trust new key`;
- file sending;
- block/unblock;
- call.

## Release Changes

Releases are made from `master`, but changes are prepared in `dev`.

Do not create a tag without an explicit decision about release version.

For patch releases, use the next available patch tag, for example `v1.0.2`, `v1.0.3`, and so on.
