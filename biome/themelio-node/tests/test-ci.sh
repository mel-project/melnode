#!/bin/bash

set -ex

TEST_DIR="$(dirname "${0}")"
PLAN_DIR="$(dirname "${TEST_DIR}")"

sudo bio pkg install --binlink core/bats
sudo bio pkg install --binlink core/nmap

source "${PLAN_DIR}/plan.sh"

echo "${BIOME_PUBLIC_KEY}" | bio origin key import

bio pkg build "${pkg_name}"

source results/last_build.env

sudo bio sup run &

sleep 5

sudo bio svc status


BIO_SVC_STATUS="$(sudo bio svc status)"
NO_SERVICES_LOADED="No services loaded."

if [ "$BIO_SVC_STATUS" == "$NO_SERVICES_LOADED" ]; then
  sudo bio pkg install --binlink --force "results/${pkg_artifact}"
  sudo bio svc load "${pkg_ident}"
else
  env HAB_BLDR_URL="https://bldr.biome.sh" sudo bio svc unload "${pkg_ident}" || true
  env HAB_BLDR_URL="https://bldr.biome.sh" sudo bio pkg install --binlink --force "results/${pkg_artifact}"
  sleep 1
  sudo bio svc load "${pkg_ident}"
fi

echo "Sleeping for 5 seconds for the service to start."
sleep 5

bats "${TEST_DIR}/test.bats"

sudo bio svc unload "${pkg_ident}" || true