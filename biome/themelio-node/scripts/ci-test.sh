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

#export SCRIPTS_DIRECTORY="$(dirname "${0}")"
#PLAN_DIRECTORY="$(dirname "${SCRIPTS_DIRECTORY}")"
#
#sudo bio pkg install --binlink themelio/bats
#sudo bio pkg install --binlink core/curl
#sudo bio pkg install --binlink core/nmap
#
#cp "${PLAN_DIRECTORY}/plan-debug.sh" "${PLAN_DIRECTORY}/plan.sh"
#
#source "${PLAN_DIRECTORY}/plan.sh"
#
#sudo bio sup run &
#
#bio pkg build "biome/${pkg_name}"
#
#source results/last_build.env
#
#sudo bio pkg install --binlink --force "results/${pkg_artifact}"
#
#sudo useradd hab -s /bin/bash -p '*'
#
#export DISABLE_HEALTH_CHECK=true
#
#sudo bio svc load "${pkg_ident}"
#
#echo "Sleeping for 5 seconds for the service to start."
#sleep 5
#
#if bats --print-output-on-failure "${SCRIPTS_DIRECTORY}/test.bats"; then
#  sudo bio svc unload "${pkg_ident}"
#else
#  sudo bio svc unload "${pkg_ident}"
#  exit 1
#fi

mkdir -p ${SCRIPTS_DIRECTORY}/packer/temporary-templates

if [ "${NETWORK_TO_BUILD}" == "mainnet" ]; then
  for region in $(cat $SCRIPTS_DIRECTORY/packer/aws_regions); do
    echo "Creating packer templates for $region-$NETWORK_TO_BUILD."

    export AWS_REGION=${region}

    envsubst < "${SCRIPTS_DIRECTORY}/packer/base-image.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/base-image-$region.pkr.hcl"
    envsubst < "${SCRIPTS_DIRECTORY}/packer/mainnet.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/mainnet-$region.pkr.hcl"
  done

  echo "Joining packer templates"
  sed -e '$s/$/\n/' -s ${SCRIPTS_DIRECTORY}/packer/temporary-templates/*.hcl > ${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp
  envsubst < "${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl"

  echo "Listing temporary-templates"
  ls -la ${SCRIPTS_DIRECTORY}/packer/temporary-templates/

  echo "Cleaning up temporary files"
  rm ${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp
  rm -rf ${SCRIPTS_DIRECTORY}/packer/temporary-templates

  echo "Validating packer template"
  packer validate "${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl"

elif [ "${NETWORK_TO_BUILD}" == "testnet" ]; then
  for region in $(cat $SCRIPTS_DIRECTORY/packer/aws_regions); do
      echo "Creating packer templates for $region-$NETWORK_TO_BUILD."

      export AWS_REGION=${region}

      envsubst < "${SCRIPTS_DIRECTORY}/packer/base-image.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/base-image-$region.pkr.hcl"
      envsubst < "${SCRIPTS_DIRECTORY}/packer/testnet.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/testnet-$region.pkr.hcl"
    done

    echo "Joining packer templates"
    sed -e '$s/$/\n/' -s ${SCRIPTS_DIRECTORY}/packer/temporary-templates/*.hcl > ${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp
    envsubst < "${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl"

    echo "Listing temporary-templates"
    ls -la ${SCRIPTS_DIRECTORY}/packer/temporary-templates/

    echo "Cleaning up temporary files"
    rm ${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp
    rm -rf ${SCRIPTS_DIRECTORY}/packer/temporary-templates

    echo "Validating packer template"
    packer validate "${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl"

else
  echo "No network specified with NETWORK_TO_BUILD. Exiting."
  exit 1
fi