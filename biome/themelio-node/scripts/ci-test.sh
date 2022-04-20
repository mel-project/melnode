#!/bin/bash

set -ex

export AWS_ACCESS_KEY_ID
export AWS_SECRET_ACCESS_KEY
export AWS_DEFAULT_REGION

export SCRIPTS_DIRECTORY="$(dirname "${0}")"
export PLAN_DIRECTORY="$(dirname "${SCRIPTS_DIRECTORY}")"
export BIOME_DIRECTORY="$(dirname "${PLAN_DIRECTORY}")"
export ROOT_DIRECTORY="$(dirname "${BIOME_DIRECTORY}")"

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

export THEMELIO_NODE_VERSION=$(cat "${ROOT_DIRECTORY}/Cargo.toml" | tomlq .package.version | tr -d '"')

if [ "${NETWORK_TO_BUILD}" == "mainnet" ]; then
  echo "Building for mainnet."
  envsubst '${THEMELIO_NODE_VERSION}' < "${PLAN_DIRECTORY}/plan-debug-mainnet.sh" > "${PLAN_DIRECTORY}/plan.sh"
  cp -r "${PLAN_DIRECTORY}/hooks-mainnet" "${PLAN_DIRECTORY}/hooks"

elif [ "${NETWORK_TO_BUILD}" == "testnet" ]; then
  echo "Building for testnet."
  envsubst '${THEMELIO_NODE_VERSION}' < "${PLAN_DIRECTORY}/plan-debug-testnet.sh" > "${PLAN_DIRECTORY}/plan.sh"
  cp -r "${PLAN_DIRECTORY}/hooks-testnet" "${PLAN_DIRECTORY}/hooks"

else
  echo "No network specified with NETWORK_TO_BUILD. Exiting."
  exit 1
fi

sudo bio pkg install --binlink themelio/bats
sudo bio pkg install --binlink core/nmap


source "${PLAN_DIRECTORY}/plan.sh"

sudo bio sup run &

#echo "Current directory in the script before the build: $(pwd)"
#echo "Contents of current directory in the script before the build: $(ls -la)"
#
#sudo bio pkg build "${PLAN_DIRECTORY}"
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
#echo "Sleeping for 10 seconds for the service to start."
#sleep 10
#
#if [ "${NETWORK_TO_BUILD}" == "mainnet" ]; then
#  if bats --print-output-on-failure "${SCRIPTS_DIRECTORY}/test-ci-mainnet.bats"; then
#    rm "${PLAN_DIRECTORY}/plan.sh"
#    rm -rf "${PLAN_DIRECTORY}/hooks"
#    sudo bio svc unload "${pkg_ident}"
#  else
#    rm "${PLAN_DIRECTORY}/plan.sh"
#    rm -rf "${PLAN_DIRECTORY}/hooks"
#    sudo bio svc unload "${pkg_ident}"
#    exit 1
#  fi
#
#elif [ "${NETWORK_TO_BUILD}" == "testnet" ]; then
#  if bats --print-output-on-failure "${SCRIPTS_DIRECTORY}/test-ci-testnet.bats"; then
#    sudo bio svc unload "${pkg_ident}"
#  else
#    sudo bio svc unload "${pkg_ident}"
#    exit 1
#  fi
#
#else
#  sudo bio svc unload "${pkg_ident}"
#  echo "No network specified with NETWORK_TO_BUILD. Exiting."
#  exit 1
#fi
#
#mkdir -p ${SCRIPTS_DIRECTORY}/packer/temporary-templates
#
#if [ "${NETWORK_TO_BUILD}" == "mainnet" ]; then
#  for region in $(cat $SCRIPTS_DIRECTORY/packer/aws_regions); do
#    echo "Creating packer templates for $region-$NETWORK_TO_BUILD."
#
#    export AWS_REGION=${region}
#
#    envsubst < "${SCRIPTS_DIRECTORY}/packer/base-image.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/base-image-$region.pkr.hcl"
#    envsubst < "${SCRIPTS_DIRECTORY}/packer/mainnet.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/mainnet-$region.pkr.hcl"
#  done
#
#  echo "Joining packer templates"
#  sed -e '$s/$/\n/' -s ${SCRIPTS_DIRECTORY}/packer/temporary-templates/*.hcl > ${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp
#  envsubst < "${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl"
#
#  echo "Cleaning up temporary files"
#  rm ${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp
#  rm -rf ${SCRIPTS_DIRECTORY}/packer/temporary-templates
#
#  echo "Validating packer mainnet template"
#  packer validate "${SCRIPTS_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl"
#
#elif [ "${NETWORK_TO_BUILD}" == "testnet" ]; then
#  for region in $(cat $SCRIPTS_DIRECTORY/packer/aws_regions); do
#      echo "Creating packer templates for $region-$NETWORK_TO_BUILD."
#
#      export AWS_REGION=${region}
#
#      envsubst < "${SCRIPTS_DIRECTORY}/packer/base-image.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/base-image-$region.pkr.hcl"
#      envsubst < "${SCRIPTS_DIRECTORY}/packer/testnet.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/packer/temporary-templates/testnet-$region.pkr.hcl"
#    done
#
#    echo "Joining packer templates"
#    sed -e '$s/$/\n/' -s ${SCRIPTS_DIRECTORY}/packer/temporary-templates/*.hcl > ${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp
#    envsubst < "${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl"
#
#    echo "Cleaning up temporary files"
#    rm ${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp
#    rm -rf ${SCRIPTS_DIRECTORY}/packer/temporary-templates
#
#    echo "Validating packer testnet template"
#    packer validate "${SCRIPTS_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl"
#
#else
#  echo "No network specified with NETWORK_TO_BUILD. Exiting."
#  exit 1
#fi