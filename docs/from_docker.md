## Using Docker
The easiest/faster option to run Selendra in Docker is to use the latest release images. These are small images that use the latest official release of the Selendra binary, pulled from our package repository.

### Install docker
To install docker see [here](https://docs.docker.com/engine/install/).

### Run docker
Let's first check the version we have and pull image from docker image. This takes a bit of time, be patient: 

```bash
docker pull laynath/selendra-chain:latest
```

You can also pass any argument/flag that Selendra supports:

```bash
docker run laynath/selendra-chain:latest --dev --name "DockerNode"
```

## Run selendra node

You can start Selendra as daemon, exposes the Selendra ports and mount a volume that will keep your blockchain data locally. Make sure that you set the ownership of your local directory to the Selendra user that is used by the container. Set user id 1000 and group id 1000, by running `chown 1000.1000 /my/local/folder -R` if you use a bind mount.

To start a Selendra node on default rpc port 9933 and default p2p port 30333 use the following command. If you want to connect to rpc port 9933 and wss port 9944, then must add Selendra startup parameter: `--rpc-external`, `-ws-external`.

Run docker node

```bash
docker run \
-d \
-p 30333:30333 \
-p 9933:9933 \
-p 9944:9944 \
-v /my/local/folder:/selendra/data/testnet \
--name testnet \
laynath/selendra-chain:latest \
--base-path selendra/data/testnet \
--chain selendra \
--rpc-external \
--ws-external \
--rpc-cors all \
--name "Dockernode" \ 
--pruning=archive
```
**_If you want to operate it on the internet. Do not expose RPC and WS ports, if they are not correctly configured._**

Insert key to selendra node

```bash
docker exec -it testnet selendra key insert \
--base-path selendra/data/testnet \
--chain selendra \
--suri <Private key> \
--password-interactive \
--key-type babe \
--scheme Sr25519

docker exec -it testnet selendra key insert \
--base-path selendra/data/testnet \
--chain selendra \
--suri <Private key> \
--password-interactive \
--key-type gran \
--scheme ed25519
```
