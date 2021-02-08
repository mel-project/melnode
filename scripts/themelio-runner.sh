#!/bin/bash

# Script Below taken from https://superuser.com/questions/181517/how-to-execute-a-command-whenever-a-file-changes
# and modified to kill the process before relaunching and also start process if its not running
# This file should be added to crontab by doing:
# crontab -e
# @reboot  /usr/local/bin/themelio-runner.sh
# or
# echo "@reboot /usr/local/bin/themelio-runner.sh" | sudo crontab -u root -
# sudo chmod 600 /var/spool/cron/crontabs/*

# NOTE: after setting up this script you can
# verify process is running using: sudo tail -f /proc/13903/fd/1 where # is pid
# You can also verify file timestamp for themelio-core is updated on each deploy using
# the stat themelio-core cmd below manually

### Set initial time of file
LTIME=`stat -c %Z /usr/local/bin/themelio-core`

while true
do
  # Store file change time
  ATIME=`stat -c %Z /usr/local/bin/themelio-core`

  # If themelio-core is not running start it
  CMD=`ps -A | grep themelio-core`
  STATUS=$?
  if [[ "$STATUS" -ne 0 ]]
  then
    echo "Starting themelio-core..."
    /usr/local/bin/themelio-core anet-node --bootstrap 94.237.109.44:11814 --listen 127.0.0.1:11814 &>> /var/log/themelio.log &
  elif [[ "$ATIME" != "$LTIME" ]]
  then
    echo "HALTING THEMELIO"
    pkill themelio-core
    echo "RUN THEMELIO IN BACKGROUND"
    # Single node for staging:
    # /usr/local/bin/themelio-core anet-node --listen 127.0.0.1:11814 &>> /var/log/themelio.org &

    # Multi-node auditors for alphanet:
    /usr/local/bin/themelio-core anet-node --bootstrap 94.237.109.44:11814 --listen 127.0.0.1:11814 &>> /var/log/themelio.log &
    LTIME=$ATIME
  else
    echo "NO CHANGE; SLEEPING for 5 seconds"
    sleep 10
  fi
done
