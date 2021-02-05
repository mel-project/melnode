#!/bin/bash

# 1. install google cloud CLI,
# 2. gcloud init
# 3. create a service account key and activate it
#    - gcloud auth activate-service-account --key-file key.json
# run this script to provision N compute instances

# create mode
# for i in $@
for i in {1..10}
do

# select randomly from region
# gcloud compute regions list or use gcloud compute zones list (latter is better)

  instance_name = ""
  zone = ""
  region = ""
  machine_type = "e2-micro"
  image = "debian-10-buster-v20210122"

  gcloud compute instances create <params>

done

# delete mode
#list all instances that begin with prefix
#get their names
#delete them all

