#!/bin/bash

set -ex

SCRIPTS_DIRECTORY="$(dirname "${0}")"
PLAN_DIRECTORY="$(dirname "${SCRIPTS_DIRECTORY}")"

sudo bio pkg install --binlink themelio/bats
sudo bio pkg install --binlink core/curl
sudo bio pkg install --binlink core/nmap

cp "${PLAN_DIRECTORY}/plan-debug.sh" "${PLAN_DIRECTORY}/plan.sh"

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

nmap 127.0.0.1 -p 11814

nmap 127.0.0.1 -p 11814 | tail -3 | head -1 | awk '{print $2}'

themelio-node --version

themelio-node --version | head -1 | awk '{print $2}'

nmap 127.0.0.1 -p 8080

nmap 127.0.0.1 -p 8080 | tail -3 | head -1 | awk '{print $2}'

curl http://127.0.0.1:8080/metrics


if bats --print-output-on-failure "${SCRIPTS_DIRECTORY}/test.bats"; then
  sudo bio svc unload "${pkg_ident}"
else
  sudo bio svc unload "${pkg_ident}"
  exit 1
fi