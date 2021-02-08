#!/bin/bash

# Use this documentation to debug through ssh: https://cloud.google.com/compute/docs/startupscript

# install package deps
yes | sudo apt update
yes | sudo apt upgrade
yes | sudo apt install git curl build-essential

# install and config rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
source $HOME/.cargo/env

# git clone (later on specify branch)
git clone https://github.com/themeliolabs/themelio-core
cd themelio-core/ || exit

# build
cargo build --release

# copy binary and runner script
cp -p ./target/release/themelio-core /usr/local/bin
cp -p ./scripts/themelio-runner.sh /usr/local/bin
#sudo chmod +x /usr/local/bin/themelio-core
#sudo chmod +x /usr/local/bin/themelio-runner.sh

# execute binary
/usr/local/bin/themelio-core anet-node --bootstrap 94.237.109.44:11814 --listen 127.0.0.1:11814 &>> /var/log/themelio.log &