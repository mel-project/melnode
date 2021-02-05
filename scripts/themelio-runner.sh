#!/bin/bash

# Script Below taken from https://superuser.com/questions/181517/how-to-execute-a-command-whenever-a-file-changes
# and modified to kill the process before relaunching
# This file should be added to crontab by doing:
# crontab -e
# @reboot  /usr/local/bin/themelio-runner.sh

# NOTE: after setting up this script you can
# verify process is running using: sudo tail -f /proc/13903/fd/1 where # is pid
# You can also verify file timestamp for themelio-core is updated on each deploy using
# the stat themelio-core cmd below manually

# TODO: Modify CI to sync this to appropriate location.  Currently file is manually copied

### Set initial time of file
LTIME=`stat -c %Z /usr/local/bin/themelio-core`

while true
do
   ATIME=`stat -c %Z /usr/local/bin/themelio-core`

   if [[ "$ATIME" != "$LTIME" ]]
   then
       echo "HALTING THEMELIO"
       pkill themelio-core
       echo "RUN THEMELIO IN BACKGROUND"
       # Single node for staging:
       # /usr/local/bin/themelio-core anet-node --listen 127.0.0.1:11814 &

       # Multi-node auditors for alphanet:
       /usr/local/bin/themelio-core anet-node --bootstrap 94.237.109.44:11814 --listen 127.0.0.1:11814 &>> /var/log/themelio.log &
       LTIME=$ATIME
   fi
   sleep 5
done
