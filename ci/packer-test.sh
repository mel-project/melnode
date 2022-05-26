#!/bin/bash

set -ex

export CI_DIRECTORY="$(dirname "${0}")"
export ROOT_DIRECTORY="$(dirname "${CI_DIRECTORY}")"


mkdir -p ${CI_DIRECTORY}/packer/temporary-templates

if [ "${NETWORK_TO_BUILD}" == "mainnet" ]; then
  for region in $(cat $CI_DIRECTORY/packer/aws_regions); do
    echo "Creating packer templates for $region-$NETWORK_TO_BUILD."

    export AWS_REGION=${region}

    envsubst < "${CI_DIRECTORY}/packer/base-image.pkr.hcl.temp" > "${CI_DIRECTORY}/packer/temporary-templates/base-image-$region.pkr.hcl"
    envsubst < "${CI_DIRECTORY}/packer/mainnet.pkr.hcl.temp" > "${CI_DIRECTORY}/packer/temporary-templates/mainnet-$region.pkr.hcl"
  done

  echo "Joining packer templates"
  sed -e '$s/$/\n/' -s ${CI_DIRECTORY}/packer/temporary-templates/*.hcl > ${CI_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp
  envsubst < "${CI_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp" > "${CI_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl"

  echo "Cleaning up temporary files"
  rm ${CI_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl.temp
  rm -rf ${CI_DIRECTORY}/packer/temporary-templates

  echo "Validating packer mainnet template"
  packer validate "${CI_DIRECTORY}/themelio-node-mainnet-debian-aws.pkr.hcl"

elif [ "${NETWORK_TO_BUILD}" == "testnet" ]; then
  for region in $(cat $CI_DIRECTORY/packer/aws_regions); do
      echo "Creating packer templates for $region-$NETWORK_TO_BUILD."

      export AWS_REGION=${region}

      envsubst < "${CI_DIRECTORY}/packer/base-image.pkr.hcl.temp" > "${CI_DIRECTORY}/packer/temporary-templates/base-image-$region.pkr.hcl"
      envsubst < "${CI_DIRECTORY}/packer/testnet.pkr.hcl.temp" > "${CI_DIRECTORY}/packer/temporary-templates/testnet-$region.pkr.hcl"
    done

    echo "Joining packer templates"
    sed -e '$s/$/\n/' -s ${CI_DIRECTORY}/packer/temporary-templates/*.hcl > ${CI_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp
    envsubst < "${CI_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp" > "${CI_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl"

    echo "Cleaning up temporary files"
    rm ${CI_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl.temp
    rm -rf ${CI_DIRECTORY}/packer/temporary-templates

    echo "Validating packer testnet template"
    packer validate "${CI_DIRECTORY}/themelio-node-testnet-debian-aws.pkr.hcl"

else
  echo "No network specified with NETWORK_TO_BUILD. Exiting."
  exit 1
fi