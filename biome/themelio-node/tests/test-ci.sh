#!/bin/bash

set -ex

TEST_DIR="$(dirname "${0}")"
PLAN_DIR="$(dirname "${TEST_DIR}")"

sudo bio pkg install --binlink core/bats
sudo bio pkg install --binlink core/nmap

source "${PLAN_DIR}/plan.sh"

sudo bio sup run &

#bio origin key download themelio
#sudo bio origin key download themelio

bio pkg build "${pkg_name}"

source results/last_build.env

sudo bio pkg install --binlink --force "results/${pkg_artifact}"

sudo useradd hab -s /bin/bash -p '*'

sudo bio svc load "${pkg_ident}"

echo "Sleeping for 5 seconds for the service to start."
sleep 5

bats "${TEST_DIR}/test.bats"

sudo bio svc unload "${pkg_ident}" || true