#!/bin/bash

set -ex

SCRIPTS_DIRECTORY="$(dirname "${0}")"
PLAN_DIRECTORY="$(dirname "${SCRIPTS_DIRECTORY}")"

sudo bio pkg install --binlink core/bats
sudo bio pkg install --binlink core/curl
sudo bio pkg install --binlink core/nmap

source "${PLAN_DIRECTORY}/plan.sh"

sudo bio sup run &

bio pkg build "biome/${pkg_name}"

source results/last_build.env

sudo bio pkg install --binlink --force "results/${pkg_artifact}"

sudo useradd hab -s /bin/bash -p '*'

export DISABLE_HEALTH_CHECK=true

sudo bio svc load "${pkg_ident}"

echo "Sleeping for 5 seconds for the service to start."
sleep 5

bats "${SCRIPTS_DIRECTORY}/test.bats"

sudo bio svc unload "${pkg_ident}" || true