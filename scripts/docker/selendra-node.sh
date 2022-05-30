#!/bin/bash

# declare OS platform & its package managers
declare -A osInfo;
osInfo[/etc/debian_version]="apt-get install -y"
osInfo[/etc/alpine-release]="apk --update add"
osInfo[/etc/centos-release]="yum install -y"
osInfo[/etc/fedora-release]="dnf install -y"
osInfo[/etc/os-release]="pacman -Syyu --noconfirm"
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

# pull testnet from docker
sudo docker image pull selendrachain/selendra-chain:latest

# create directory for selendra-chaindb
read -p "Name a directory where the Selendra Chain will store: " selendradb
mkdir -p ${HOME}/${selendradb}

# allow selendra-chaindb (blockchain data) access to local directory
sudo chown 1000.1000 /home/$USER/${selendradb} -R

# name container and node
read -p "What should the container call?: " x
read -p "What do you want to call your node?:" y

# run docker container
sudo docker container run \
    --network="host" \
    --name ${x} \
    -v /home/$USER/${selendradb}:/selendra/data \
    selendrachain/selendra-chain:latest \
    --base-path selendra/data \
    --chain selendra \
    --port 30333 \
    --rpc-port 9933 \
    --ws-port 9944 \
    --telemetry-url "wss://telemetry.polkadot.io/submit/ 0" \
    --validator \
    --name ${y} \
    --bootnodes /ip4/157.245.56.213/tcp/30333/p2p/12D3KooWDLR899Spcx4xJ3U8cZttv9zjzJoey3HKaTZiNqwojZJB

# restart docker
# sudo docker restart ${container}

# to check your node go >>> https://telemetry.polkadot.io/#list/0x3d7efe9e36b20531f2a735feac13f3cad96798b2d9036a6950dac8076c19c545

# to become a validator use this command to get your Session key.
# curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "author_rotateKeys", "params":[]}' http://localhost:9933

# output will look like >>> {"jsonrpc":"2.0","result":"0x45e81ef5c...0615265", "id":1} 
# copy only  >>> 0x45e81ef5c...0615265
# then go to testnet.selendra.org and follow this instruction >>> https://github.com/selendra/selendra-chain/blob/main/docs/validator.md
