# Contributing

## Homebrew tap secret

The [release workflow](.github/workflows/release.yml) publishes a GitHub Release,
then updates [`fearlessfara/homebrew-tap`](https://github.com/fearlessfara/homebrew-tap).
`GITHUB_TOKEN` cannot push to a different repository, so add this secret on
`fearlessfara/envie`:

| Secret | Purpose |
| --- | --- |
| `HOMEBREW_TAP_TOKEN` | PAT (classic or fine-grained) with **Contents: Write** on `fearlessfara/homebrew-tap` |

Create a dedicated token (fine-grained PAT with Contents: Write on the tap, or a
classic PAT with `repo`), then:

```bash
gh secret set HOMEBREW_TAP_TOKEN --repo fearlessfara/envie
```

A `HOMEBREW_TAP_TOKEN` is already set on `fearlessfara/envie`. Replace it with a
dedicated PAT if GitHub CLI auth is revoked or expires — the value currently
there came from `gh auth token` and is not a long-lived PAT.

Without the secret, tagged releases still publish the binary; the formula is left
unchanged and the workflow logs a warning.

Do not push a `v*` tag until this release workflow is on the default branch.

## First release checklist

1. `fearlessfara/homebrew-tap` exists with `Formula/envie.rb`.
2. `HOMEBREW_TAP_TOKEN` is set on `fearlessfara/envie`.
3. The release workflow is on the default branch.
4. `Cargo.toml` `version` matches the tag (for example `0.1.0` → `v0.1.0`).
5. Push the tag. Confirm the Release asset exists, the formula SHA updated, and:

   ```bash
   brew install fearlessfara/tap/envie
   ```
