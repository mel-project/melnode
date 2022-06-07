#!/bin/bash

if [ "${NETWORK}" = 'mainnet' ]; then
  if [ -n "${ADVERTISE_OFF}" ]; then
    themelio-node --database /var/lib/themelio-node/main --listen 0.0.0.0:11814
  elif [ -n "${ADVERTISE_MANUAL}" ]; then
    themelio-node --database /var/lib/themelio-node/main --listen 0.0.0.0:11814 --advertise "${ADVERTISE_MANUAL}":11814
  else
    PUBLIC_IP_ADDRESS="$(curl -s http://checkip.amazonaws.com)"
    themelio-node --database /var/lib/themelio-node/main --listen 0.0.0.0:11814 --advertise "${PUBLIC_IP_ADDRESS}":11814
  fi

elif [ "${NETWORK}" = 'testnet' ]; then
  if [ -n "${ADVERTISE_OFF}" ]; then
    themelio-node --database /var/lib/themelio-node/main --testnet --bootstrap tm-1.themelio.org:11814
  elif [ -n "${ADVERTISE_MANUAL}" ]; then
    themelio-node --database /var/lib/themelio-node/main --testnet --bootstrap tm-1.themelio.org:11814 --advertise "${ADVERTISE_MANUAL}":11814
  else
    PUBLIC_IP_ADDRESS="$(curl -s http://checkip.amazonaws.com)"
    themelio-node --database /var/lib/themelio-node/main --testnet --bootstrap tm-1.themelio.org:11814 --advertise "${PUBLIC_IP_ADDRESS}":11814
  fi

else
  echo "No network specified with NETWORK."
  echo "Defaulting to mainnet."

    if [ -n "${ADVERTISE_OFF}" ]; then
      themelio-node --database /var/lib/themelio-node/main --listen 0.0.0.0:11814
    elif [ -n "${ADVERTISE_MANUAL}" ]; then
      themelio-node --database /var/lib/themelio-node/main --listen 0.0.0.0:11814 --advertise "${ADVERTISE_MANUAL}":11814
    else
      PUBLIC_IP_ADDRESS="$(curl -s http://checkip.amazonaws.com)"
      themelio-node --database /var/lib/themelio-node/main --listen 0.0.0.0:11814 --advertise "${PUBLIC_IP_ADDRESS}":11814
    fi
fi