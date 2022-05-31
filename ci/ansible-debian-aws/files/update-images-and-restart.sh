#!/bin/bash

set -e


if [ "${NETWORK}" == "mainnet" ]; then
  IMAGE=ghcr.io/themeliolabs/themelio-node-mainnet
  CURRENT_CONTAINER_ID=$(docker ps | grep ${IMAGE} | awk '{print $1}')
elif [ "${NETWORK}" == "testnet" ]; then
  IMAGE=ghcr.io/themeliolabs/themelio-node-testnet
  CURRENT_CONTAINER_ID=$(docker ps | grep ${IMAGE} | awk '{print $1}')
else
  echo "NETWORK not specified. Not running update."
  exit 1
fi


echo "Image is: ${IMAGE}"

echo "Running container ID is: ${CURRENT_CONTAINER_ID}"

echo "Pulling latest image"
docker pull "${IMAGE}" > /dev/null 2>&1

CURRENT_IMAGE_SHA=$(docker inspect --format "{{.Image}}" "${CURRENT_CONTAINER_ID}")

LATEST_IMAGE_SHA=$(docker inspect --format "{{.Id}}" "${IMAGE}")

echo "Current image SHA is: ${CURRENT_IMAGE_SHA}"

echo "Latest image SHA is: ${LATEST_IMAGE_SHA}"



if [ "${CURRENT_IMAGE_SHA}" != "${LATEST_IMAGE_SHA}" ];then
  echo "Upgrading to the latest image."
  systemctl restart themelio-node

  echo "Removing old images."
  docker image prune -a -f
else
  echo "Already running the latest image."
fi