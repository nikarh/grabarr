# Releasing

## How a release happens

1. Run the **Create a release PR** workflow (Actions → Create a release PR → patch / minor / major). It bumps the
   version in `Cargo.toml`, regenerates `CHANGELOG.md` with git-cliff from the conventional commits, and opens a pull
   request titled `Release vX.Y.Z` with the release notes as its body.
2. Review and merge it. The **Release** workflow tags the merge commit and creates the GitHub release with the notes.
3. The **Publish** workflow builds the static x86_64 binary (musl, frontend embedded), wraps it in the `FROM scratch`
   image, checks the image runs read-only without capabilities, pushes it to `ghcr.io/nikarh/grabarr` as `X.Y.Z`,
   `X.Y` and `latest`, writes `SHA256SUMS` and its GPG signature, and uploads everything to the release.

Every push to `main` also updates the `latest` pre-release (the **Latest** workflow): the tag moves to the new
commit, the notes list the unreleased changes, and Publish uploads the same files, signed the same way, with the
version marked as a build of main (`0.1.1-dev+abc1234`, from `GRABARR_VERSION_SUFFIX`), and pushes the image as `main`
and `sha-<commit>`. The pre-release is never marked as GitHub's latest release, so `releases/latest` keeps pointing at
the newest `vX.Y.Z`.

Asset names never change, so `https://github.com/nikarh/grabarr/releases/latest/download/<name>` always points at the
newest file:

| Asset | Contents |
| --- | --- |
| `grabarr-x86_64-unknown-linux-musl` | The static binary |
| `grabarr-x86_64-unknown-linux-musl.tar.gz` | The binary, the example config, the README and the licenses |
| `SHA256SUMS`, `SHA256SUMS.asc` | Checksums of all of the above and their detached GPG signature |

Dependabot's pull requests are merged by the **Auto-merge** workflow once every required check is green.

## One-time setup

### Release bot and the `release` environment

The release workflows act as a GitHub App (`nikarh-release-bot`), like the other nikarh repositories: the app needs
*Contents* and *Pull requests* read & write, and to be installed on the repository. The `release` environment
(Settings → Environments) only allows the `main` and `release` branches and `v*` tags, and holds these secrets:

| Secret | Value |
| --- | --- |
| `APP_CLIENT_ID` | The app's client ID (`Iv…`, on the app's settings page) |
| `APP_PRIVATE_KEY` | A private key of the app (PEM) |
| `GPG_PRIVATE_KEY` | ASCII armored private key that signs `SHA256SUMS` |
| `GPG_PASSPHRASE` | Its passphrase |

```sh
gh secret set APP_CLIENT_ID --env release --repo nikarh/grabarr --body <client id>
gh secret set APP_PRIVATE_KEY --env release --repo nikarh/grabarr < release-bot.private-key.pem
```

### GPG signing key

"grabarr releases" (`990AAEA980944EA7C73CDCD4FCD6BDF753A0EDA4`, ed25519, until 2029-11-06) is in the maintainer's GPG
keyring, its passphrase in the desktop keyring (`secret-tool lookup service grabarr-release-gpg`). The public key is
[`signing-key.asc`](signing-key.asc). To set the secrets again:

```sh
secret-tool lookup service grabarr-release-gpg | gh secret set GPG_PASSPHRASE --env release --repo nikarh/grabarr
secret-tool lookup service grabarr-release-gpg \
  | gpg --batch --pinentry-mode loopback --passphrase-fd 0 --armor \
    --export-secret-keys 990AAEA980944EA7C73CDCD4FCD6BDF753A0EDA4 \
  | gh secret set GPG_PRIVATE_KEY --env release --repo nikarh/grabarr
```
