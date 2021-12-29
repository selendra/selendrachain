#!/bin/bash

# declare OS platform & its package managers
declare -A osInfo;
osInfo[/etc/os-release]="pacman -Syy -y"
osInfo[/etc/debian_version]="apt-get install -y"
osInfo[/etc/alpine-release]="apk --update add"
osInfo[/etc/centos-release]="yum install -y"
osInfo[/etc/fedora-release]="dnf install -y"
osInfo[/etc/os-release]="brew install -y"

for f in ${!osInfo[@]}
do
    if [[ -f $f ]];then
        package_manager=${osInfo[$f]}
    fi
done

# list packages to install
package="git"
package="docker"
package="docker.id"

# install packages 
${package_manager} ${package}

# pull testnet from docker
sudo docker pull laynath/selendra-chain:test

# create directory for selendra-chaindb
mkdir -p /home/$USER/selendra-chaindb

# allow selendra-chaindb (blockchain data) access to local directory
sudo chown 1000.1000 /home/$USER/selendra-chaindb -R

# name container and node
read -p "What should the container call?: " $container

read -p "What do you want to call your node?: $node"

# remove any duplicate containers
sudo docker container rm $container
# run docker container
sudo docker container run \
--network="host" \
--name $container \
-v /home/rithy/selendrachaindb:/selendra/data/testnet \
laynath/selendra-chain:test \
--base-path selendra/data/testnet \
--chain testnet \
--port 30333 \
--rpc-port 9933 \
--ws-port 9944 \
--telemetry-url "wss://telemetry.polkadot.io/submit/ 0" \
--validator \
--name $node

#restart docker
sudo docker restart $container

# use this command to get your Session key.
# curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "author_rotateKeys", "params":[]}' http://localhost:9933>


