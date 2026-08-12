# Contributing

## Releasing

Same steps every time. Do not skip the changelog, and do not invent a second
way to ship.

1. `master` is green.
2. Dogfood an example (below). A dry-run is the minimum; an apply must be
   destroyed in the same session.
3. Move `[Unreleased]` in [CHANGELOG.md](CHANGELOG.md) to `## [x.y.z] - YYYY-MM-DD`
   and leave a fresh empty Unreleased section. Set `version` in `Cargo.toml`
   to the same `x.y.z`.
4. Commit, then tag and push:

   ```bash
   git tag v0.2.2
   git push origin v0.2.2
   ```

5. The [release workflow](.github/workflows/release.yml) builds a universal
   macOS binary and static Linux binaries for x86_64 and aarch64, attaches
   them to the GitHub Release with their checksums, and updates
   [`fearlessfara/homebrew-tap`](https://github.com/fearlessfara/homebrew-tap).
   The same tag is what workflows pin as `uses: fearlessfara/envie@v0.2.2`.
6. Confirm the Release assets exist, `brew upgrade fearlessfara/tap/envie`
   installs that version, and CI's `action` job is still green on `master`.

Publishing the Action to the GitHub Marketplace is a browser step on that
release (2FA). The API cannot do it. Skip it if you only need
`uses: fearlessfara/envie@<tag>`.

Do not push a `v*` tag until the release workflow is on the default branch.

## Dogfooding

CI never applies. Before a release, run the cheapest example against a real
account so deploy and delete have been seen to work together.

[`examples/static-site/02-envie`](examples/static-site/02-envie) is the one:
one root, S3 only, names prefixed `envie-test-`, region `eu-west-1`.

```bash
cargo build --release
cd examples/static-site/02-envie

../../../target/release/envie deploy --env pr-dogfood --dry-run

# Optional: apply, then always tear down in this session.
aws-vault exec personal --no-session -- \
  ../../../target/release/envie deploy --env pr-dogfood --no-prompt
aws-vault exec personal --no-session -- \
  ../../../target/release/envie delete --env pr-dogfood --no-prompt
```

Do not leave the environment up. Do not use NAT, RDS, or anything with an
hourly charge. Details and flags live in that example's README.

## Homebrew tap secret

The release workflow updates
[`fearlessfara/homebrew-tap`](https://github.com/fearlessfara/homebrew-tap).
`GITHUB_TOKEN` cannot push to a different repository, so add this secret on
`fearlessfara/envie`:

| Secret | Purpose |
| --- | --- |
| `HOMEBREW_TAP_TOKEN` | PAT (classic or fine-grained) with **Contents: Write** on `fearlessfara/homebrew-tap` |

```bash
gh secret set HOMEBREW_TAP_TOKEN --repo fearlessfara/envie
```

Without the secret, tagged releases still publish the binary; the formula is
left unchanged and the workflow logs a warning.
