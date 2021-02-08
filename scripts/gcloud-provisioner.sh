#!/bin/bash

# Run this script to provision N compute instances
# ./gcloud-provision create 10
# ./gcloud-provisioner delete
#
# Before running this script:
# 1. Install google cloud CLI,
# 2. > gcloud init
# 3. Create a service account key and activate it
#    - gcloud auth activate-service-account --key-file key.json

PREFIX="themelio"
ARGS=("$@")
MODE=${ARGS[0]}
if [[ "$MODE" == "create" ]]
then
  NUM=${ARGS[1]}
  if [[ "$NUM" =~ ^-?[0-9]*[.,]?[0-9]*[eE]?-?[0-9]+$ ]]
  then
    # Get an array of all the zones
    # (skip first row which contains column names and kept only first col)
    ZONES=(`gcloud compute zones list | awk 'FNR > 1 { print $1 }'`)

    # TODO: create instance template here and provision and clone nodes from here

    for i in $(seq "$NUM")
    do
      RAND_ZONE_INDEX=$[$RANDOM % ${#ZONES[@]}]
      RAND_ZONE=${ZONES[$RAND_ZONE_INDEX]}
      MACHINE_TYPE="e2-micro"
      MACHINE_NAME=${PREFIX}-${RAND_ZONE}-${i}

      # create a compute instance in a random zone and launch startup script
      echo "Creating and provisioning ${MACHINE_IMAGE} with themelio-core..."
      yes | gcloud compute instances create $MACHINE_NAME --zone ${RAND_ZONE} --machine-type ${MACHINE_TYPE} --metadata-from-file startup-script=gcloud-startup-script.sh --async
    done

  else
    echo "Must input a number for the number of nodes to create."
  fi
elif [[ "$MODE" == "delete" ]]
then
  NAME_TO_ZONES=(`gcloud compute instances list | awk 'FNR > 1 { print $1 ";" $2 }'`)
  for i in "${NAME_TO_ZONES[@]}"
  do
    arr=(${i//;/ })
    NAME=${arr[0]}
    ZONE=${arr[1]}
    if [[ "$NAME" == *"$PREFIX"* ]]; then
      echo "Delete ${NAME} in ${ZONE}"
      yes | gcloud compute instances delete $NAME --zone=$ZONE
    fi
  done
else
  echo "invalid option"
fi
