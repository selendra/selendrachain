#!/bin/bash

# declare OS platform & its package managers
declare -A osInfo;
osInfo[/etc/debian_version]="apt-get install -y"
osInfo[/etc/alpine-release]="apk --update add"
osInfo[/etc/centos-release]="yum install -y"
osInfo[/etc/fedora-release]="dnf install -y"
osInfo[/etc/os-release]="pacman -S --noconfirm"
# osInfo[/etc/os-release]="brew install -y"

for f in ${!osInfo[@]}
do
    if [[ -f $f ]];then
        package_manager=${osInfo[$f]}
    fi
done

# list packages to install
package="git"
if [[ ${package_manager} == "pacman -Syyu --noconfirm" ]]; then 
    package+=" docker noto-fonts-emoji"
else
    package+=" docker.io"
fi

sudo $package_manager $package

# enable and start docker service
sudo systemctl enable docker.service
sudo systemctl start docker.service

# pull testnet from docker
sudo docker pull selendrachain/selendra-chain:latest

# create directory for selendra-chaindb
read -p "Name a directory where the Selendra Chain will store: " i
mkdir -p ${HOME}/${USER}/${i}

# allow selendra-chaindb (blockchain data) access to local directory
sudo chown 1000.1000 ${HOME}/${USER}/${i} -R

# name container and node
read -p "What should the container call?: " x
read -p "What do you want to call your node?:" y

# run docker container
sudo docker container run \
    --network="host" \
    --name ${x} \
    -v ${HOME}/${USER}/${i}:/selendra/data/cardamom \
    selendrachain/selendra-chain:latest \
    --base-path selendra/data/cardamom \
    --chain cardamom \
    --port 30333 \
    --rpc-port 9933 \
    --ws-port 9944 \
    --telemetry-url "wss://telemetry.polkadot.io/submit/ 0" \
    --validator \
    --name ${y}

# restart docker
sudo docker restart ${x}

# to check your node go >>> https://telemetry.polkadot.io/#list/0x889494a97f9573ead42f297ac4b91935cf9727b1bdae29fd4ba56bc8468767c7

# to become a validator use this command to get your Session key.
# curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "author_rotateKeys", "params":[]}' http://localhost:9933

# output will look like >>> {"jsonrpc":"2.0","result":"0x45e81ef5c...0615265", "id":1} 
# copy only  >>> 0x45e81ef5c...0615265
# then go to testnet.selendra.org and follow this instruction >>> https://github.com/selendra/selendra-chain/blob/main/docs/validator.md

