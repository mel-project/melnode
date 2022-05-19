#!/bin/bash

set -e

BASE_IMAGE="registry"
REGISTRY="registry.hub.docker.com"
IMAGE="$REGISTRY/$BASE_IMAGE"
CID=$(docker ps | grep $IMAGE | awk '{print $1}')
docker pull $IMAGE

for im in $CID
do
    LATEST=`docker inspect --format "{{.Id}}" $IMAGE`
    RUNNING=`docker inspect --format "{{.Image}}" $im`
    NAME=`docker inspect --format '{{.Name}}' $im | sed "s/\///g"`
    echo "Latest:" $LATEST
    echo "Running:" $RUNNING
    if [ "$RUNNING" != "$LATEST" ];then
        echo "upgrading $NAME"
        stop docker-$NAME
        docker rm -f $NAME
        start docker-$NAME
    else
        echo "$NAME up to date"
    fi
done



#ExecStartPre=/usr/local/bin/docker-compose -f /home/admin/compose/docker-compose.yml down
#ExecStart=/usr/bin/bash -c 'HOSTNAME=$HOSTNAME exec /usr/local/bin/docker-compose -f /home/admin/compose/docker-compose.yml up'
#ExecStop=/usr/local/bin/docker-compose -f /home/admin/compose/docker-compose.yml down


# The above is an example from here: https://stackoverflow.com/questions/26423515/how-to-automatically-update-your-docker-containers-if-base-images-are-updated
#
# I need to adapt this, update the ansible/packer playbook to copy it into /usr/local/bin/, then have that be run through a systemd timer every 30 seconds
#
# Look here for the service/timer solution: https://stackoverflow.com/a/53557536