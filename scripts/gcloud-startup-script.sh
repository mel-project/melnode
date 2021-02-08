#!/bin/bash

# Use this documentation to debug through ssh: https://cloud.google.com/compute/docs/startupscript
# specifically this command shows far along the script is: sudo journalctl -u google-startup-scripts.service

# Set bin permissions so we can SCP files
sudo chmod 777 /usr/local/bin/

# Setup cronjob to wait for runner script
echo "@reboot /usr/local/bin/themelio-runner.sh" | sudo crontab -u root -

# Ensure crontabs have correct permission levels
sudo chmod 600 /var/spool/cron/crontabs/*
