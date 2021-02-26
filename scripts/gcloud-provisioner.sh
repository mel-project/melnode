#!/bin/bash

# Run this script to provision N compute instances
# ./gcloud-provision create 10
# ./gcloud-provisioner delete
#
# Before running this script:
# 0. Need gcc-musl dep (for mac see: https://www.andrew-thorburn.com/cross-compiling-a-simple-rust-web-app/)
# 1. Install google cloud CLI,
# 2. run: gcloud init
# 3. Create a service account key from console and get the key to activate it with:
#    - gcloud auth activate-service-account --key-file key.json

PREFIX="themelio"
ARGS=("$@")
MODE=${ARGS[0]}
if [[ "$MODE" == "create" ]]
then
  # Create cross-compiled binary
  TARGET_CC=x86_64-linux-musl-gcc RUSTFLAGS="-C linker=x86_64-linux-musl-gcc" cargo build --target=x86_64-unknown-linux-musl --release

  NUM=${ARGS[1]}
  if [[ "$NUM" =~ ^-?[0-9]*[.,]?[0-9]*[eE]?-?[0-9]+$ ]]
  then
    # Get an array of all the zones
    # (skip first row which contains column names and kept only first col)
    ZONES=(`gcloud compute zones list | awk 'FNR > 1 { print $1 }'`)

    # TODO: create instance template here and provision and clone nodes from here

    for i in $(seq "$NUM")
    do
      echo "Creating and provisioning ${MACHINE_IMAGE} with themelio-core..."

      RAND_ZONE_INDEX=$[$RANDOM % ${#ZONES[@]}]
      RAND_ZONE=${ZONES[$RAND_ZONE_INDEX]}
      MACHINE_TYPE="e2-micro"
      MACHINE_NAME=${PREFIX}-${RAND_ZONE}-${i}

      # Four tasks are run in sequence (the whole job is async):
      # 1. create a compute instance in a random zone with launch startup script
      # 2. Upload cross-compiled binary and runner script (30 sec sleep to ensure startup script done)
      # 3. Clean stop instance
      # 4. Clean start instance
      (yes | gcloud compute instances create ${MACHINE_NAME} --zone ${RAND_ZONE} --machine-type e2-micro --metadata-from-file startup-script=gcloud-startup-script.sh) \
      && (sleep 30s && gcloud compute scp ../target/x86_64-unknown-linux-musl/release/themelio-core themelio-runner.sh ${MACHINE_NAME}:/usr/local/bin --zone ${RAND_ZONE}) \
      && (gcloud compute instances stop ${MACHINE_NAME} --zone ${RAND_ZONE}) \
      && (gcloud compute instances start ${MACHINE_NAME} --zone ${RAND_ZONE}) &

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
      yes | gcloud compute instances delete $NAME --zone=$ZONE &
    fi
  done
else
  echo "invalid option"
fi

echo "waiting for jobs to complete..."
for job in `jobs -p`
do
  echo $job
  wait $job
done
echo "jobs completed"