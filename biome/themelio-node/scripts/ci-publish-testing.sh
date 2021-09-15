#!/bin/bash

set -ex

export SCRIPTS_DIRECTORY="$(dirname "${0}")"
PLAN_DIRECTORY="$(dirname "${SCRIPTS_DIRECTORY}")"

cp "${PLAN_DIRECTORY}/plan-release.sh" "${PLAN_DIRECTORY}/plan.sh"

source "${PLAN_DIRECTORY}/plan.sh"


bio pkg build "biome/${pkg_name}"

source results/last_build.env

hart_file="results/${pkg_artifact}"


echo "Publishing artifact to the stable channel"

ls -la results

bio pkg upload --auth "${HABITAT_AUTH_TOKEN}" --url "${HAB_BLDR_URL}" "${hart_file}"


echo "Exporting docker image"
sudo bio pkg export container "${hart_file}"

source results/last_container_export.env

for tag in ${tags//,/ }; do
  local_tag="ghcr.io/themeliolabs/themelio-node:${tag}"

  docker tag "${name}:${tag}" "${local_tag}"

	docker push "${local_tag}"
done