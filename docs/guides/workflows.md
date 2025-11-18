# Common Workflows

This guide covers common development workflows using Envie.

## Workflow 1: Feature Branch Development

Deploy your feature to an isolated environment.

### Scenario

You're working on a new authentication feature in a branch called `feature-auth`.

### Steps

```bash
# 1. Create your feature branch
git checkout -b feature-auth

# 2. Deploy to ephemeral environment (use stable database for speed)
envie deploy --unit api --env feature-auth \
  -E database:stable.sandbox \
  -E networking:stable.sandbox

# 3. Make code changes
# ... edit your Terraform/application code ...

# 4. Redeploy to see changes
envie deploy --unit api --env feature-auth

# 5. When done, clean up
envie delete --env feature-auth
```

### Benefits

- ✅ Isolated testing environment
- ✅ Reuses stable database (faster, cheaper)
- ✅ Easy cleanup when done

---

## Workflow 2: Pull Request Preview

Automatically deploy PR environments in CI/CD.

### Scenario

Automatically create an environment for each pull request.

### CI/CD Script

```yaml
# .github/workflows/pr-preview.yml
name: PR Preview

on:
  pull_request:
    types: [opened, synchronize]

jobs:
  preview:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Envie
        run: |
          # Install envie
          cargo install envie

      - name: Deploy Preview
        env:
          AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
          AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
        run: |
          # Deploy to PR-specific environment
          envie deploy --unit api --env pr-${{ github.event.pull_request.number }} \
            -E database:stable.sandbox \
            --no-prompt

      - name: Comment on PR
        uses: actions/github-script@v6
        with:
          script: |
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.name,
              body: '🚀 Preview deployed to environment: pr-${{ github.event.pull_request.number }}'
            })
```

### Cleanup

```yaml
# .github/workflows/pr-cleanup.yml
name: PR Cleanup

on:
  pull_request:
    types: [closed]

jobs:
  cleanup:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Destroy Preview
        run: |
          envie delete --env pr-${{ github.event.pull_request.number }} --no-prompt
```

---

## Workflow 3: Testing Against Production Data

Test your code changes against production data (read-only).

### Scenario

You need to test performance with production data volume.

### Steps

```bash
# 1. Deploy with production database (ensure Lambda has read-only permissions!)
envie deploy --unit api --env prod-test \
  -E database:stable.production \
  -E networking:ephemeral

# 2. Run your tests
# ... performance tests ...

# 3. Clean up
envie delete --env prod-test
```

### ⚠️ Safety Considerations

1. **Read-only permissions**: Ensure your Lambda has only `SELECT` permissions
2. **Rate limiting**: Add throttling to avoid impacting production
3. **Short-lived**: Delete immediately after testing
4. **Monitor**: Watch production metrics during testing

---

## Workflow 4: Full Integration Testing

Deploy everything to an isolated environment.

### Scenario

Testing a major refactor that touches multiple services.

### Steps

```bash
# 1. Deploy all services to isolated environment
envie deploy --unit services --env integration-test

# 2. Run integration tests
# ... run test suite ...

# 3. Check results
envie output --env integration-test --format json > results.json

# 4. Clean up
envie delete --env integration-test
```

### Benefits

- ✅ Complete isolation
- ✅ Tests real cross-service interactions
- ✅ No impact on shared environments

---

## Workflow 5: Staged Deployment

Deploy changes progressively through environments.

### Scenario

Rolling out a database migration safely.

### Steps

```bash
# 1. Deploy to dev environment
envie deploy --unit database --env dev-migration

# ... test ...

# 2. Deploy to sandbox (stable)
envie deploy --unit database --env sandbox
# (Requires workspace.envie.yaml update for stable environment)

# ... test with team ...

# 3. Deploy to production
envie deploy --unit database --env production

# ... monitor ...
```

### Best Practices

1. **Always test in dev first**
2. **Get team validation in sandbox**
3. **Deploy during low-traffic windows**
4. **Have rollback plan ready**

---

## Workflow 6: Developer Personal Sandbox

Each developer has their own environment.

### Scenario

Multiple developers working on the same codebase.

### Setup

```bash
# Each developer uses their name as environment ID

# Alice
envie deploy --unit api --env alice \
  -E database:stable.sandbox

# Bob
envie deploy --unit api --env bob \
  -E database:stable.sandbox

# Charlie
envie deploy --unit api --env charlie \
  -E database:stable.sandbox
```

### Benefits

- ✅ No conflicts between developers
- ✅ Shared stable database (consistent data)
- ✅ Independent iteration speed

---

## Workflow 7: Debugging Production Issues

Reproduce production issues in a safe environment.

### Scenario

Production bug that's hard to reproduce.

### Steps

```bash
# 1. Create debug environment with production state
envie deploy --unit api --env debug-issue-123 \
  -E database:stable.production \
  --dry-run  # Preview first!

# 2. Add debug logging (deploy with changes)
# ... edit code to add logging ...

envie deploy --unit api --env debug-issue-123 \
  -E database:stable.production

# 3. Trigger the bug
# ... reproduce the issue ...

# 4. Analyze logs
aws logs tail /aws/lambda/myapp-debug-issue-123-api --follow

# 5. Clean up immediately
envie delete --env debug-issue-123
```

### ⚠️ Important

- Only use read-only access to production data
- Delete environment immediately after debugging
- Consider data privacy regulations

---

## Workflow 8: Performance Testing

Load testing without affecting production.

### Scenario

Testing system capacity before a product launch.

### Steps

```bash
# 1. Deploy with production-like config
envie deploy --unit api --env load-test \
  -E database:stable.sandbox  # Use sandbox with production-like data

# 2. Scale up resources (edit Terraform)
# ... increase Lambda concurrency, etc. ...

envie deploy --unit api --env load-test

# 3. Run load tests
# ... k6, locust, etc. ...

# 4. Collect metrics
envie output --env load-test --format json > metrics.json

# 5. Clean up
envie delete --env load-test
```

---

## Workflow 9: Migration Testing

Test migrations before running on production.

### Scenario

Database schema migration that needs validation.

### Steps

```bash
# 1. Create test environment
envie deploy --unit database --env migration-test

# 2. Run migration
# ... apply migration ...

# 3. Test with production snapshot (optional)
# Restore production snapshot to test database

# 4. Test application with migrated schema
envie deploy --unit api --env migration-test \
  -E database:ephemeral  # Use the migrated test database

# 5. Validate
# ... run tests ...

# 6. Clean up
envie delete --env migration-test
```

---

## Workflow 10: Cost Optimization

Identify and clean up unused environments.

### Steps

```bash
# 1. List all environments
envie list

# Example output:
# 📦 api (Component)
#    Workspaces:
#      • myapp-dev-123
#      • myapp-old-feature  ← Haven't used in weeks
#      • myapp-alice

# 2. Review old environments
# Check last deployment time, cost, etc.

# 3. Delete unused environments
envie delete --env old-feature

# 4. Set up automated cleanup (CI/CD)
# Delete environments older than X days
```

---

## Quick Reference

| Use Case | Command Pattern |
|----------|----------------|
| Feature development | `envie deploy --unit api --env feature-name -E database:stable.sandbox` |
| PR preview | `envie deploy --unit api --env pr-123 --no-prompt` |
| Integration test | `envie deploy --unit services --env integration-test` |
| Production debugging | `envie deploy --unit api --env debug-issue -E database:stable.production` (read-only!) |
| Performance testing | `envie deploy --unit api --env load-test` |
| Cleanup | `envie delete --env <name>` |

---

## Best Practices Summary

1. **Use descriptive environment names**: `feature-auth`, not `test1`
2. **Preview before deploying**: `envie plan` first
3. **Reuse stable resources**: Use `-E` to share databases
4. **Clean up regularly**: Delete old environments
5. **Automate in CI/CD**: Deploy on PR, cleanup on merge
6. **Monitor costs**: Review AWS bills for unused resources
7. **Document custom workflows**: Add to team runbook

---

**Previous**: [Environment Overrides](environment-overrides.md) | **Next**: [CI/CD Integration](cicd-integration.md)
