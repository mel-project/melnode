#!/bin/bash

set -ex

export SCRIPTS_DIRECTORY="$(dirname "${0}")"
PLAN_DIRECTORY="$(dirname "${SCRIPTS_DIRECTORY}")"

source "${PLAN_DIRECTORY}/plan.sh"


bio pkg build "${pkg_name}"

source results/last_build.env

hart_file="results/${pkg_artifact}"


echo "Publishing artifact to the stable channel"

bio pkg upload --auth "${HABITAT_AUTH_TOKEN}" --url "${HAB_BLDR_URL}" "${hart_file}" -c stable
bio pkg upload --auth "${BIOME_AUTH_TOKEN}" --url "${BIOME_BLDR_URL}" "${hart_file}" -c stable


echo "Exporting docker image"
sudo bio pkg export container "${hart_file}"

source results/last_container_export.env

for tag in ${tags//,/ }; do
  local_tag="ghcr.io/themeliolabs/themelio-node:${tag}"

  docker tag "${name}:${tag}" "${local_tag}"

	docker push "${local_tag}"
done


echo "Creating packer images"

## We have these run with `&` so that they run in parallel. The openstack CLI is horrifically slow.
#
## Flavour per region
#env OS_REGION_NAME=BHS5 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID' > BHS5_FLAVOUR.output &
#env OS_REGION_NAME=DE1 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID' > DE1_FLAVOUR.output &
#env OS_REGION_NAME=GRA5 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID' > GRA5_FLAVOUR.output &
#env OS_REGION_NAME=GRA11 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID' > GRA11_FLAVOUR.output &
#env OS_REGION_NAME=SBG5 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID' > SBG5_FLAVOUR.output &
#env OS_REGION_NAME=UK1 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID' > UK1_FLAVOUR.output &
#env OS_REGION_NAME=WAW1 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID' > WAW1_FLAVOUR.output &
#
### Image ID per region
#env OS_REGION_NAME=BHS5 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID' > BHS5_IMAGE_ID.output &
#env OS_REGION_NAME=DE1 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID' > DE1_IMAGE_ID.output &
#env OS_REGION_NAME=GRA5 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID' > GRA5_IMAGE_ID.output &
#env OS_REGION_NAME=GRA11 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID' > GRA11_IMAGE_ID.output &
#env OS_REGION_NAME=SBG5 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID' > SBG5_IMAGE_ID.output &
#env OS_REGION_NAME=UK1 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID' > UK1_IMAGE_ID.output &
#env OS_REGION_NAME=WAW1 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID' > WAW1_IMAGE_ID.output &
#
### Network ID per region
#env OS_REGION_NAME=BHS5 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID' > BHS5_NETWORK_ID.output &
#env OS_REGION_NAME=DE1 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID' > DE1_NETWORK_ID.output &
#env OS_REGION_NAME=GRA5 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID' > GRA5_NETWORK_ID.output &
#env OS_REGION_NAME=GRA11 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID' > GRA11_NETWORK_ID.output &
#env OS_REGION_NAME=SBG5 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID' > SBG5_NETWORK_ID.output &
#env OS_REGION_NAME=UK1 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID' > UK1_NETWORK_ID.output &
#env OS_REGION_NAME=WAW1 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID' > WAW1_NETWORK_ID.output &
#
#wait
#
## Export Flavours
#export BHS5_FLAVOUR=$(cat BHS5_FLAVOUR.output)
#export DE1_FLAVOUR=$(cat DE1_FLAVOUR.output)
#export GRA5_FLAVOUR=$(cat GRA5_FLAVOUR.output)
#export GRA11_FLAVOUR=$(cat GRA11_FLAVOUR.output)
#export SBG5_FLAVOUR=$(cat SBG5_FLAVOUR.output)
#export UK1_FLAVOUR=$(cat UK1_FLAVOUR.output)
#export WAW1_FLAVOUR=$(cat WAW1_FLAVOUR.output)
#
## Export Image IDs
#export BHS5_IMAGE_ID=$(cat BHS5_IMAGE_ID.output)
#export DE1_IMAGE_ID=$(cat DE1_IMAGE_ID.output)
#export GRA5_IMAGE_ID=$(cat GRA5_IMAGE_ID.output)
#export GRA11_IMAGE_ID=$(cat GRA11_IMAGE_ID.output)
#export SBG5_IMAGE_ID=$(cat SBG5_IMAGE_ID.output)
#export UK1_IMAGE_ID=$(cat UK1_IMAGE_ID.output)
#export WAW1_IMAGE_ID=$(cat WAW1_IMAGE_ID.output)
#
## Export Network IDs
#export BHS5_NETWORK_ID=$(cat BHS5_NETWORK_ID.output)
#export DE1_NETWORK_ID=$(cat DE1_NETWORK_ID.output)
#export GRA5_NETWORK_ID=$(cat GRA5_NETWORK_ID.output)
#export GRA11_NETWORK_ID=$(cat GRA11_NETWORK_ID.output)
#export SBG5_NETWORK_ID=$(cat SBG5_NETWORK_ID.output)
#export UK1_NETWORK_ID=$(cat UK1_NETWORK_ID.output)
#export WAW1_NETWORK_ID=$(cat WAW1_NETWORK_ID.output)
#
#envsubst < "${SCRIPTS_DIRECTORY}/themelio-node.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/themelio-node.pkr.hcl"
#
#packer validate "${SCRIPTS_DIRECTORY}/themelio-node.pkr.hcl"
#
#packer build "${SCRIPTS_DIRECTORY}/themelio-node.pkr.hcl"