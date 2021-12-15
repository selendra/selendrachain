 ### Install & Configure Network Time Protocol (NTP) Client
 NTP is a networking protocol designed to synchronize the clocks of computers over a network. NTP allows you to synchronize the clocks of all the systems within the network. Currently it is required that validators' local clocks stay reasonably in sync, so you should be running NTP or a similar service.

 ```sh
# Check if NTP is installed and running, you should see System clock synchronized: yes 
timedatectl
# If you do not see it, you can install it by executing
sudo apt-get install ntp
```

***WARNING***: Skipping this can result in the validator node missing block authorship opportunities. If the clock is out of sync (even by a small amount), the blocks the validator produces may not get accepted by the network. This will result in ImOnline heartbeats making it on chain, but zero allocated blocks making it on chain.

### Synchronize Chain Data and Run validator

**Note**: By default, Validator nodes are in archive mode. If you've already synced the chain not in archive mode, you must first remove the database with **selendra purge-chain** and then ensure that you run **Selendra** with the *--pruning=archive* option.
Note that an archive node and non-archive node's databases are not compatible with each other, and to switch you will need to purge the chain data.
The *--pruning=archive* flag is implied by the *--validator* flag, so it is only required explicitly if you start your node without one of these two options. If you do not set your pruning to archive node, even when not running in validator mode, you will need to re-sync your database when you switch.

You can begin syncing your node by running the following command:

```sh
./target/release/selendra \
  --base-path <save path> \
  --chain selendra \
  --pruning=archive \
  --bootnodes /ip4/<IP Address>/tcp/<p2p Port>/p2p/<Peer ID>
```
Depending on the size of the chain when you do this, this step may take anywhere from a few minutes to a few hours.
After sync finish chain data stop it and running the following command:

```sh
./target/release/selendra \
--base-path <save path> \
--chain selendra \
--port 30333 \
--ws-port 9944 \
--rpc-port 9933 \
--telemetry-url "wss://telemetry.polkadot.io/submit/ 0" \
--validator \
--name <Name> \
--bootnodes /ip4/<IP Address>/tcp/<p2p Port>/p2p/<Peer ID>
```
