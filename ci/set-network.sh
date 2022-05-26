#!/bin/bash

set -ex

if [ "${GITHUB_BASE_REF}" == "master" ]; then
  echo "NETWORK_TO_BUILD=mainnet" >> $GITHUB_ENV

elif [ "${GITHUB_BASE_REF}" == "testnet" ]; then
  echo "NETWORK_TO_BUILD=testnet" >> $GITHUB_ENV

else
  echo "Not running against deployment branches. Exiting."
  exit 0
fi