#!/bin/bash

set -ex

export SCRIPTS_DIRECTORY="$(dirname "${0}")"

echo "Creating packer images"

sed -e '$s/$/\n/' -s ${SCRIPTS_DIRECTORY}/packer/*.hcl > ${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl.temp

envsubst < "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl"

rm themelio-node-debian-aws.pkr.hcl.temp

packer validate "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl"

packer build "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl"