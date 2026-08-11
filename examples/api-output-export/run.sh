#!/usr/bin/env bash
# Deploy the API, export Terraform outputs three ways, run JS smoke tests, destroy.
#
# Usage (from this directory):
#   aws-vault exec personal --no-session -- ./run.sh demo-1
#
# Requires: envie on PATH (or ENVIE=path/to/envie), terraform, aws CLI, Node 20+.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

ENV_ID="${1:-demo-1}"
ENVIE="${ENVIE:-envie}"
REGION="${AWS_REGION:-eu-west-1}"
BUCKET="envie-test-api-export-tfstate"
LOCK_TABLE="envie-test-api-export-tflocks"

bootstrap_backend() {
  if ! aws s3api head-bucket --bucket "$BUCKET" 2>/dev/null; then
    echo "Creating state bucket s3://$BUCKET ..."
    if [[ "$REGION" == "us-east-1" ]]; then
      aws s3api create-bucket --bucket "$BUCKET"
    else
      aws s3api create-bucket \
        --bucket "$BUCKET" \
        --create-bucket-configuration "LocationConstraint=$REGION"
    fi
    aws s3api put-bucket-versioning \
      --bucket "$BUCKET" \
      --versioning-configuration Status=Enabled
    aws s3api put-public-access-block \
      --bucket "$BUCKET" \
      --public-access-block-configuration \
      BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=true,RestrictPublicBuckets=true
  fi

  if ! aws dynamodb describe-table --table-name "$LOCK_TABLE" --region "$REGION" &>/dev/null; then
    echo "Creating lock table $LOCK_TABLE ..."
    aws dynamodb create-table \
      --table-name "$LOCK_TABLE" \
      --attribute-definitions AttributeName=LockID,AttributeType=S \
      --key-schema AttributeName=LockID,KeyType=HASH \
      --billing-mode PAY_PER_REQUEST \
      --region "$REGION" >/dev/null
    aws dynamodb wait table-exists --table-name "$LOCK_TABLE" --region "$REGION"
  fi
}

echo "==> Bootstrapping remote state ($BUCKET / $LOCK_TABLE)"
bootstrap_backend

echo "==> Deploying environment $ENV_ID"
"$ENVIE" deploy --env "$ENV_ID" --no-prompt

echo "==> Exporting outputs (env / json / yaml)"
"$ENVIE" output --env "$ENV_ID" --format env -f .env
"$ENVIE" output --env "$ENV_ID" --format json -f outputs.json
"$ENVIE" output --env "$ENV_ID" --format yaml -f outputs.yaml

echo "==> Running smoke tests"
set -a
# shellcheck disable=SC1091
source .env
set +a
export OUTPUTS_DIR="$ROOT"
(cd tests && npm test)

echo "==> Destroying environment $ENV_ID"
"$ENVIE" destroy --env "$ENV_ID" --no-prompt

echo "Done. Exported files left behind: .env outputs.json outputs.yaml (gitignored)."
