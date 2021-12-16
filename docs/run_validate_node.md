

### Synchronize Chain Data and Run validator

**Note**: By default, Validator nodes are in archive mode. If you've already synced the chain not in archive mode, you must first remove the database with **selendra purge-chain** and then ensure that you run **Selendra** with the *--pruning=archive* option.
Note that an archive node and non-archive node's databases are not compatible with each other, and to switch you will need to purge the chain data.
The *--pruning=archive* flag is implied by the *--validator* flag, so it is only required explicitly if you start your node without one of these two options. If you do not set your pruning to archive node, even when not running in validator mode, you will need to re-sync your database when you switch.

You can begin syncing your node by running the following command:

```sh
./target/release/selendra \
  --base-path <save path> \
  --chain selendra \
  --pruning=archive
```
Depending on the size of the chain when you do this, this step may take anywhere from a few minutes to a few hours.
Once your node is fully synced, stop the process by pressing Ctrl-C. At your terminal prompt, you will now start running the node.

**Note**: You can give your validator any name that you like, but note that others will be able to see it, and it will be included in the list of all servers using the same telemetry server. Since numerous people are using telemetry, it is recommended that you choose something likely to be unique.

```sh
./target/release/selendra \
--base-path <save path> \
--chain selendra \
--port 30333 \
--ws-port 9944 \
--rpc-port 9933 \
--telemetry-url "wss://telemetry.polkadot.io/submit/ 0" \
--validator \
--name <Name>
```
