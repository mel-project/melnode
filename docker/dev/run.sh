#!/bin/bash

if [ "${NETWORK}" = 'mainnet' ]; then
  PUBLIC_IP_ADDRESS="$(curl -s http://checkip.amazonaws.com)"
  themelio-node --database /var/lib/themelio-node/main --listen 0.0.0.0:11814 --advertise "${PUBLIC_IP_ADDRESS}":11814 &
  sleep 3
  bats --print-output-on-failure /tmp/ci.bats
  exit 0
elif [ "${NETWORK}" = 'testnet' ]; then
  PUBLIC_IP_ADDRESS="$(curl -s http://checkip.amazonaws.com)"
  themelio-node --database /var/lib/themelio-node/main --testnet --bootstrap tm-1.themelio.org:11814 --advertise "${PUBLIC_IP_ADDRESS}":11814
  sleep 3
  bats --print-output-on-failure /tmp/ci.bats
  exit 0
else
  echo "No network specified with NETWORK. Exiting."
  exit 1
fi