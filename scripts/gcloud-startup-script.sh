#!/bin/bash

# install package deps
yes | sudo apt update
yes | sudo apt upgrade
yes | sudo apt install git curl

# install rust
yes | curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# git clone (later on specify branch)
git clone https://github.com/themeliolabs/themelio-core
cd themelio-core || exit

# build
cargo build --release

# copy binary and runner script
cp ./target/release/themelio-core /usr/local/bin
#cp ./scripts/themelio-runner.sh /usr/local/bin
#sudo chmod +x /usr/local/bin/themelio-core
#sudo chmod +x /usr/local/bin/themelio-runner.sh

# execute binary
/usr/local/bin/themelio-core anet-node --bootstrap 94.237.109.44:11814 --listen 127.0.0.1:11814 &>> /var/log/themelio.log &