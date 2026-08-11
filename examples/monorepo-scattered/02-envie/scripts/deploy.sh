#!/usr/bin/env bash
# The script this repository used before Envie: every root module, in order,
# with the environment repeated by hand in three places.
set -euo pipefail

ENVIRONMENT="${1:?usage: deploy.sh <environment>}"

for stack in platform/network services/api/terraform; do
  pushd "$stack" >/dev/null
  terraform init
  terraform apply -var "environment=${ENVIRONMENT}" -var "env=${ENVIRONMENT}"
  popd >/dev/null
done
