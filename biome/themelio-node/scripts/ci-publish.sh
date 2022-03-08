#!/bin/bash

set -ex

export AWS_ACCESS_KEY_ID
export AWS_SECRET_ACCESS_KEY
export AWS_DEFAULT_REGION

export SCRIPTS_DIRECTORY="$(dirname "${0}")"
PLAN_DIRECTORY="$(dirname "${SCRIPTS_DIRECTORY}")"

if [ -z "${LOKI_USERNAME}" ]; then
  echo "The LOKI_USERNAME environment variable must be set."
  echo "Exiting."

  exit 1
fi

if [ -z "${LOKI_PASSWORD}" ]; then
  echo "The LOKI_PASSWORD environment variable must be set."
  echo "Exiting."

  exit 1
fi

if [ "${NETWORK_TO_BUILD}" == "mainnet" ]; then
  cp "${PLAN_DIRECTORY}/plan-debug-mainnet.sh" "${PLAN_DIRECTORY}/plan.sh"
  cp -r "${PLAN_DIRECTORY}/hooks-mainnet" "${PLAN_DIRECTORY}/hooks"

elif [ "${NETWORK_TO_BUILD}" == "testnet" ]; then
  cp "${PLAN_DIRECTORY}/plan-debug-testnet.sh" "${PLAN_DIRECTORY}/plan.sh"
  cp -r "${PLAN_DIRECTORY}/hooks-testnet" "${PLAN_DIRECTORY}/hooks"

else
  echo "No network specified with NETWORK_TO_BUILD. Exiting."
  exit 1
fi


source "${PLAN_DIRECTORY}/plan.sh"

bio pkg build "${PLAN_DIRECTORY}"

source results/last_build.env

hart_file="results/${pkg_artifact}"

mkdir -p ${SCRIPTS_DIRECTORY}/packer/temporary-templates


if [ "${NETWORK_TO_BUILD}" == "mainnet" ]; then
  echo "Publishing mainnet artifact to the stable channel"

  bio pkg upload --auth "${HABITAT_AUTH_TOKEN}" --url "${HAB_BLDR_URL}" "${hart_file}" -c stable

  # Uploading to the biome builder is currently broken.
  #This can be uncommented when this bug report is remedied: https://github.com/biome-sh/biome/issues/14
  #bio pkg upload --auth "${BIOME_AUTH_TOKEN}" --url "${BIOME_BLDR_URL}" "${hart_file}" -c stable


  echo "Exporting mainnet docker image"
  sudo bio pkg export container "${hart_file}"

  source results/last_container_export.env

  for tag in ${tags//,/ }; do
    local_tag="ghcr.io/themeliolabs/themelio-node-mainnet:${tag}"

    docker tag "${name}:${tag}" "${local_tag}"

  	docker push "${local_tag}"
  done


  for region in $(cat $SCRIPTS_DIRECTORY/packer/aws_regions); do
    echo "Creating packer templates for $region-$NETWORK_TO_BUILD."

    export AWS_REGION=${region}

    envsubst < "${SCRIPTS_DIRECTORY}/packer/base-image.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/base-image-$region.pkr.hcl"
    envsubst < "${SCRIPTS_DIRECTORY}/packer/mainnet.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/mainnet-$region.pkr.hcl"
  done

  echo "Joining packer templates"
  sed -e '$s/$/\n/' -s ${SCRIPTS_DIRECTORY}/packer/temporary-templates/*.hcl > ${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp
  envsubst < "${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl"

  echo "Cleaning up temporary files"
  rm ${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp
  rm -rf ${SCRIPTS_DIRECTORY}/packer/temporary-templates

  echo "Validating packer mainnet template"
  packer validate "${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl"

  echo "Building packer mainnet images"
  packer build "${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl"

elif [ "${NETWORK_TO_BUILD}" == "testnet" ]; then
  echo "Publishing testnet artifact to the stable channel"

  bio pkg upload --auth "${HABITAT_AUTH_TOKEN}" --url "${HAB_BLDR_URL}" "${hart_file}" -c stable

  # Uploading to the biome builder is currently broken.
  #This can be uncommented when this bug report is remedied: https://github.com/biome-sh/biome/issues/14
  #bio pkg upload --auth "${BIOME_AUTH_TOKEN}" --url "${BIOME_BLDR_URL}" "${hart_file}" -c stable


  echo "Exporting testnet docker image"
  sudo bio pkg export container "${hart_file}"

  source results/last_container_export.env

  for tag in ${tags//,/ }; do
    local_tag="ghcr.io/themeliolabs/themelio-node-testnet:${tag}"

    docker tag "${name}:${tag}" "${local_tag}"

    docker push "${local_tag}"
  done

  for region in $(cat $SCRIPTS_DIRECTORY/packer/aws_regions); do
      echo "Creating packer templates for $region-$NETWORK_TO_BUILD."

      export AWS_REGION=${region}

      envsubst < "${SCRIPTS_DIRECTORY}/packer/base-image.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/base-image-$region.pkr.hcl"
      envsubst < "${SCRIPTS_DIRECTORY}/packer/testnet.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/testnet-$region.pkr.hcl"
    done

    echo "Joining packer templates"
    sed -e '$s/$/\n/' -s ${SCRIPTS_DIRECTORY}/packer/temporary-templates/*.hcl > ${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp
    envsubst < "${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl"

    echo "Cleaning up temporary files"
    rm ${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp
    rm -rf ${SCRIPTS_DIRECTORY}/packer/temporary-templates

    echo "Validating packer testnet template"
    packer validate "${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl"

    echo "Building packer testnet images"
    packer build "${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl"

else
  echo "No network specified with NETWORK_TO_BUILD. Exiting."
  exit 1
fi