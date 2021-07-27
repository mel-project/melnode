#!/bin/bash

set -ex

PLAN_DIR="$(dirname "${0}")"

source "${PLAN_DIR}/plan.sh"


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

# We have these run with `&` so that they run in parallel. The openstack CLI is horrifically slow.

# Flavour per region
export BHS5_FLAVOUR="$(env OS_REGION_NAME=BHS5 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID')" &
export DE1_FLAVOUR="$(env OS_REGION_NAME=DE1 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID')" &
export GRA5_FLAVOUR="$(env OS_REGION_NAME=GRA5 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID')" &
export GRA11_FLAVOUR="$(env OS_REGION_NAME=GRA11 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID')" &
export SBG5_FLAVOUR="$(env OS_REGION_NAME=SBG5 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID')" &
export UK1_FLAVOUR="$(env OS_REGION_NAME=UK1 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID')" &
export WAW1_FLAVOUR="$(env OS_REGION_NAME=WAW1 openstack flavor list -f json | jq -r '.[] | select(.Name == "d2-2") | .ID')" &

# Image ID per region
export BHS5_IMAGE_ID="$(env OS_REGION_NAME=BHS5 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID')" &
export DE1_IMAGE_ID="$(env OS_REGION_NAME=DE1 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID')" &
export GRA5_IMAGE_ID="$(env OS_REGION_NAME=GRA5 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID')" &
export GRA11_IMAGE_ID="$(env OS_REGION_NAME=GRA11 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID')" &
export SBG5_IMAGE_ID="$(env OS_REGION_NAME=SBG5 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID')" &
export UK1_IMAGE_ID="$(env OS_REGION_NAME=UK1 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID')" &
export WAW1_IMAGE_ID="$(env OS_REGION_NAME=WAW1 openstack image list -f json | jq -r '.[] | select(.Name == "Archlinux") | .ID')" &

# Network ID per region
export BHS5_NETWORK_ID="$(env OS_REGION_NAME=BHS5 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID')" &
export DE1_NETWORK_ID="$(env OS_REGION_NAME=DE1 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID')" &
export GRA5_NETWORK_ID="$(env OS_REGION_NAME=GRA5 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID')" &
export GRA11_NETWORK_ID="$(env OS_REGION_NAME=GRA11 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID')" &
export SBG5_NETWORK_ID="$(env OS_REGION_NAME=SBG5 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID')" &
export UK1_NETWORK_ID="$(env OS_REGION_NAME=UK1 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID')" &
export WAW1_NETWORK_ID="$(env OS_REGION_NAME=WAW1 openstack network list -f json | jq -r '.[] | select(.Name == "Ext-Net") | .ID')" &

wait

envsubst < themelio-node.pkr.hcl.temp > themelio-node.pkr.hcl

packer validate themelio-node.pkr.hcl

packer build themelio-node.pkr.hcl