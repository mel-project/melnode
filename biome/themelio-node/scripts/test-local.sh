#!/bin/bash

set -ex

export SCRIPTS_DIRECTORY="$(dirname "${0}")"
export PLAN_DIRECTORY="$(dirname "${SCRIPTS_DIRECTORY}")"
export BIOME_DIRECTORY="$(dirname "${PLAN_DIRECTORY}")"
export ROOT_DIRECTORY="$(dirname "${BIOME_DIRECTORY}")"


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

bio pkg install --binlink themelio/bats
bio pkg install --binlink core/curl
bio pkg install --binlink core/nmap

pushd "${PLAN_DIRECTORY}"

source "plan.sh"

if [ -n "${SKIP_BUILD}" ]; then
  source "results/last_build.env"

  BIO_SVC_STATUS="$(bio svc status)"
  NO_SERVICES_LOADED="No services loaded."

  if [ "$BIO_SVC_STATUS" == "$NO_SERVICES_LOADED" ]; then
    bio pkg install --binlink --force "results/${pkg_artifact}"
    bio svc load "${pkg_ident}"
  else
    bio svc unload "${pkg_ident}" || true
    bio pkg install --binlink --force "results/${pkg_artifact}"
    sleep 1
    bio svc load "${pkg_ident}"
  fi
else
  build

  source "results/last_build.env"

  BIO_SVC_STATUS="$(bio svc status)"
  NO_SERVICES_LOADED="No services loaded."

  if [ "$BIO_SVC_STATUS" == "$NO_SERVICES_LOADED" ]; then
    bio pkg install --binlink --force "results/${pkg_artifact}"
    bio svc load "${pkg_ident}"
  else
    bio svc unload "${pkg_ident}" || true
    bio pkg install --binlink --force "results/${pkg_artifact}"
    sleep 1
    bio svc load "${pkg_ident}"
  fi
fi

echo "Sleeping for 10 seconds for the service to start."
sleep 10

if [ "${NETWORK_TO_BUILD}" == "mainnet" ]; then
  if bats --print-output-on-failure "scripts/test-local-mainnet.bats"; then
    rm "plan.sh"
    rm -rf hooks
    bio svc unload "${pkg_ident}"
  else
    rm "plan.sh"
    rm -rf hooks
    bio svc unload "${pkg_ident}"
    exit 1
  fi

elif [ "${NETWORK_TO_BUILD}" == "testnet" ]; then
  if bats --print-output-on-failure "scripts/test-local-testnet.bats"; then
    rm "plan.sh"
    rm -rf hooks
    bio svc unload "${pkg_ident}"
  else
    rm "plan.sh"
    rm -rf hooks
    bio svc unload "${pkg_ident}"
    exit 1
  fi

else
  rm "plan.sh"
  rm -rf hooks
  bio svc unload "${pkg_ident}"
  echo "No network specified with NETWORK_TO_BUILD. Exiting."
  exit 1
fi

popd