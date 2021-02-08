#!/bin/bash

# Use this documentation to debug through ssh: https://cloud.google.com/compute/docs/startupscript
# specifically this command shows far along the script is: sudo journalctl -u google-startup-scripts.service

# install package deps (disabling udpate/upgrade since it takes so long)
#yes | sudo apt update
#yes | sudo apt upgrade
yes | sudo apt install git curl build-essential

# install and config rust as root
curl https://sh.rustup.rs -sSf | sh -s -- -y
export PATH="/root/.cargo/bin:$PATH"

# clone repo
sudo -s
cd /root || exit
git clone https://github.com/themeliolabs/themelio-core
cd ./themelio-core/ || exit

# start cron job (waits for binary)
#sudo chmod +x ./scripts/themelio-runner.sh
#cp -p ./scripts/themelio-runner.sh /usr/local/bin
# (crontab -l 2>/dev/null; echo "*/5 * * * * /usr/local/bin/themelio-runner.sh -with args") | crontab -

# build
sudo /root/.cargo/bin/cargo build --release

# copy binary and runner script
cp -p ./target/release/themelio-core /usr/local/bin

# execute binary
sudo /usr/local/bin/themelio-core anet-node --bootstrap 94.237.109.44:11814 --listen 127.0.0.1:11814 &>> /var/log/themelio.log &