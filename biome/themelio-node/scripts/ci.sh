#!/bin/bash

set -ex

TEST_DIR="$(dirname "${0}")"
PLAN_DIR="$(dirname "${TEST_DIR}")"

sudo bio pkg install --binlink core/bats
sudo bio pkg install --binlink core/nmap

source "${PLAN_DIR}/plan.sh"

sudo bio sup run &

bio pkg build "${pkg_name}"

source results/last_build.env

sudo bio pkg install --binlink --force "results/${pkg_artifact}"

sudo useradd hab -s /bin/bash -p '*'

sudo bio svc load "${pkg_ident}"

echo "Sleeping for 5 seconds for the service to start."
sleep 5

bats "${TEST_DIR}/test.bats"

sudo bio svc unload "${pkg_ident}" || true


echo "Exporting docker image"

sudo bio pkg export container "results/${pkg_artifact}"

source results/last_container_export.env

for name_tag in ${name_tags//,/ }; do
	docker push "ghcr.io/themelio/themelio-node/${name_tag}"
done

#id=75a6708c4147
#name=themelio/themelio-node
#tags=latest,0.1.0,0.1.0-20210721183904
#name_tags=themelio/themelio-node:latest,themelio/themelio-node:0.1.0,themelio/themelio-node:0.1.0-20210721183904