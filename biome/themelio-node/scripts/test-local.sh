#!/bin/bash

set -e

TESTDIR="$(dirname "${0}")"
PLANDIR="$(dirname "${TESTDIR}")"

bio pkg install --binlink core/bats
bio pkg install --binlink core/nmap

source "${PLANDIR}/plan.sh"

if [ -n "${SKIP_BUILD}" ]; then
  source results/last_build.env

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
  pushd "${PLANDIR}"
  build
  popd

  source results/last_build.env

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

echo "Sleeping for 5 seconds for the service to start."
sleep 5

bats "${TESTDIR}/test.bats"

bio svc unload "${pkg_ident}" || true