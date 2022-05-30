#!/bin/bash

# In any Linux distros, we must update the system first before installing any packages,
# or it break the system.
sudo apt update -y
sudo apt upgrade -y

# install docker
sudo apt install -y git docker.io
# add user to docker group
sudo gpasswd -a $USER docker

# enable docker service to autostart docker on system boot
sudo systemctl enable --now docker.service

# pull testnet from docker
sudo docker pull selendrachain/selendra-chain:latest

# create directory for selendra-chaindb
read -p "Name a directory where the Selendra Chain will store: " i
mkdir -p ${HOME}/${i}

# allow selendra-chaindb (blockchain data) access to local directory
sudo chown 1000.1000 ${HOME}/${i} -R

# name container and node
read -p "What should the container call?: " x
read -p "What do you want to call your node?:" y

# run docker container
sudo docker container run \
    -d \
    --network="host" \
    --name ${x} \
    -v ${HOME}/${i}:/selendra/data/testnet \
    selendrachain/selendra-chain:latest \
    --base-path selendra/data/testnet \
    --chain testnet \
    --port 30333 \
    --rpc-port 9933 \
    --ws-port 9944 \
    --telemetry-url "wss://telemetry.polkadot.io/submit/ 0" \
    --validator \
    --name ${y}

# restart docker
sudo docker restart ${x}
