#!/bin/bash

set -ex

export AWS_ACCESS_KEY_ID
export AWS_SECRET_ACCESS_KEY
export AWS_DEFAULT_REGION

if [ -z "${PROMTAIL_USERNAME}" ]; then
  echo "The PROMTAIL_USERNAME environment variable must be set."
  echo "Exiting."

  exit 1
fi

if [ -z "${PROMTAIL_PASSWORD}" ]; then
  echo "The PROMTAIL_PASSWORD environment variable must be set."
  echo "Exiting."

  exit 1
fi

export SCRIPTS_DIRECTORY="$(dirname "${0}")"
PLAN_DIRECTORY="$(dirname "${SCRIPTS_DIRECTORY}")"

cp "${PLAN_DIRECTORY}/plan-release.sh" "${PLAN_DIRECTORY}/plan.sh"

source "${PLAN_DIRECTORY}/plan.sh"


bio pkg build "biome/${pkg_name}"

source results/last_build.env

hart_file="results/${pkg_artifact}"


echo "Publishing artifact to the stable channel"

bio pkg upload --auth "${HABITAT_AUTH_TOKEN}" --url "${HAB_BLDR_URL}" "${hart_file}" -c stable

# Uploading to the biome builder is currently broken.
#This can be uncommented when this bug report is remedied: https://github.com/biome-sh/biome/issues/14
#bio pkg upload --auth "${BIOME_AUTH_TOKEN}" --url "${BIOME_BLDR_URL}" "${hart_file}" -c stable


echo "Exporting docker image"
sudo bio pkg export container "${hart_file}"

source results/last_container_export.env

for tag in ${tags//,/ }; do
  local_tag="ghcr.io/themeliolabs/themelio-node:${tag}"

  docker tag "${name}:${tag}" "${local_tag}"

	docker push "${local_tag}"
done


mkdir -p ${SCRIPTS_DIRECTORY}/packer/temporary-templates

for region in $(cat $SCRIPTS_DIRECTORY/packer/aws_regions); do
  echo "Creating packer templates for the $region region."

  export AWS_REGION=${region}

  envsubst < "${SCRIPTS_DIRECTORY}/packer/mainnet.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/mainnet-$region.pkr.hcl"
  envsubst < "${SCRIPTS_DIRECTORY}/packer/testnet.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/testnet-$region.pkr.hcl"
done

cp ${SCRIPTS_DIRECTORY}/packer/00-base-image.pkr.hcl ${SCRIPTS_DIRECTORY}/packer/temporary-templates/

echo "Joining packer templates"
sed -e '$s/$/\n/' -s ${SCRIPTS_DIRECTORY}/packer/temporary-templates/*.hcl > ${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl.temp
envsubst < "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl"

echo "Cleaning up temporary files"
rm ${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl.temp
rm -rf ${SCRIPTS_DIRECTORY}/packer/temporary-templates

echo "Validating packer template"
packer validate "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl"

echo "Building packer images"
packer build "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl"