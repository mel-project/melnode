#!/bin/bash

set -ex

export SCRIPTS_DIRECTORY="$(dirname "${0}")"

echo "Joining packer templates"

sed -e '$s/$/\n/' -s ${SCRIPTS_DIRECTORY}/packer/*.hcl > ${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl.temp

envsubst < "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl.temp" > "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl"

rm themelio-node-debian-aws.pkr.hcl.temp

echo "Validating packer template"

packer validate "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl"

echo "Building packer images"

packer build "${SCRIPTS_DIRECTORY}/themelio-node-debian-aws.pkr.hcl"