#!/bin/bash

set -ex

export SCRIPTS_DIRECTORY="$(dirname "${0}")"
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

if bats --print-output-on-failure "${SCRIPTS_DIRECTORY}/test.bats"; then
  sudo bio svc unload "${pkg_ident}"
else
  sudo bio svc unload "${pkg_ident}"
  exit 1
fi

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