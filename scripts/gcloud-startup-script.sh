#!/bin/bash

# install package deps
sudo apt update && sudo apt upgrade
sudo apt install git curl

# install rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# git clone (later on specify branch)
git clone https://github.com/themeliolabs/themelio-core
cd themelio-core || exit

# build
cargo build --release

# copy binary and runner script
cp ./target/release/themelio-core /usr/local/bin
#cp ./scripts/themelio-runner.sh /usr/local/bin
#chmod +x /usr/local/bin/themelio-core
#chmod +x /usr/local/bin/themelio-runner.sh

#add cronjob
# may not be needed: https://www.golinuxcloud.com/create-schedule-cron-job-shell-script-linux/
#restart

# execute binary
/usr/local/bin/themelio-core anet-node --bootstrap 94.237.109.44:11814 --listen 127.0.0.1:11814 &>> /var/log/themelio.log &