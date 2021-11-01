# Run a Validator on Selendra-Chain
This guide will instruct you how to set up a validator node on the **Selendra** network.

## How many SEL do I need ?​
 To became **Selendra** validator you need to stake at least **31416 SEL**. This stake amount can come from yourself or from nominators. This means that as a minimum, you will need enough **SEL** to set up Stash and Controller accounts with the existential deposit, plus a little extra for transaction fees. The rest can come from nominators.
 
 ***Warning***: Any **SEL** that you stake for your validator is liable to be slashed, meaning that an insecure or improper setup may result in loss of **SEL** tokens! If you are not confident in your ability to run a validator node, it is recommended to nominate your SEL to a trusted validator node instead.
 
 ##  Build Selendra-Chain
 To Install and build Selendra-Chain see [here](https://github.com/selendra/selendra-chain/blob/main/README.md).
 
 ## Install & Configure Network Time Protocol (NTP) Client
 NTP is a networking protocol designed to synchronize the clocks of computers over a network. NTP allows you to synchronize the clocks of all the systems within the network. Currently it is required that validators' local clocks stay reasonably in sync, so you should be running NTP or a similar service.

 ```sh
# Check if NTP is installed and running, you should see System clock synchronized: yes 
timedatectl
# If you do not see it, you can install it by executing
sudo apt-get install ntp
```

***WARNING***: Skipping this can result in the validator node missing block authorship opportunities. If the clock is out of sync (even by a small amount), the blocks the validator produces may not get accepted by the network. This will result in ImOnline heartbeats making it on chain, but zero allocated blocks making it on chain.

## Synchronize Chain Data

**Note**: By default, Validator nodes are in archive mode. If you've already synced the chain not in archive mode, you must first remove the database with **selendra purge-chain** and then ensure that you run **Selendra** with the *--pruning=archive* option.
Note that an archive node and non-archive node's databases are not compatible with each other, and to switch you will need to purge the chain data.
The *--pruning=archive* flag is implied by the *--validator* flag, so it is only required explicitly if you start your node without one of these two options. If you do not set your pruning to archive node, even when not running in validator mode, you will need to re-sync your database when you switch.

You can begin syncing your node by running the following command:

```sh
./target/release/selendra \
  --chain selendra \
  --pruning=archive \
  --bootnodes /ip4/<IP Address>/tcp/<p2p Port>/p2p/<Peer ID>
```
Depending on the size of the chain when you do this, this step may take anywhere from a few minutes to a few hours.

## Bond SEL

It is highly recommended that you make your controller and stash accounts be two separate accounts. For this, you will create two accounts and make sure each of them have at least enough funds to pay the fees for making transactions. Keep most of your funds in the stash account since it is meant to be the custodian of your staking funds.
Make sure not to bond all your **SEL** balance since you will be unable to pay transaction fees from your bonded balance.
It is now time to set up our validator. We will do the following:

Bond the **SEL** of the Stash account. These **SEL** will be put at stake for the security of the network and can be slashed.
Select the Controller. This is the account that will decide when to start or stop validating.

First, go to the Staking section. Click on "Account Actions", and then the "+ Stash" button.

![image](assets/bond_sel.png)

- Stash account - Select your Stash account. In this example, we will bond 31,416 SEL, where the minimum bonding amount is 31,416. Make sure that your Stash account contains at least this much. You can, of course, stake more than this.
- Controller account - Select the Controller account created earlier. This account will also need a small amount of SEL in order to start and stop validating.
- Value bonded - How much SEL from the Stash account you want to bond/stake. Note that you do not need to bond all of the SEL in that account. Also note that you can always bond more SEL later. However, withdrawing any bonded amount requires the duration of the unbonding period.
- Payment destination - The account where the rewards from validating are sent. Payouts can go to any custom address. If you'd like to redirect payments to an account that is neither the controller nor the stash account, set one up. Note that it is extremely unsafe to set an exchange address as the recipient of the staking rewards.

Once everything is filled in properly, click Bond and sign the transaction with your Stash account.

![image](assets/authorize.png)

Your bonded account will available under Stashes. You should now see a new card with all your accounts. The bonded amount on the right corresponds to the funds bonded by the Stash account.

![image](assets/aferSucess.png)

## Set Session Keys
Once your node is fully synced, stop the process by pressing Ctrl-C. At your terminal prompt, you will now start running the node.

```sh
./target/release/selendra \
  --chain selendra \
  --validator \
  --name "name on telemetry" \
  --bootnodes /ip4/<IP Address>/tcp/<p2p Port>/p2p/<Peer ID>
```

**Note**: You can give your validator any name that you like, but note that others will be able to see it, and it will be included in the list of all servers using the same telemetry server. Since numerous people are using telemetry, it is recommended that you choose something likely to be unique.

## Generating the Session Keys

Once ensuring that you have connected to your node, the easiest way to set session keys for your node is by calling the author_rotateKeys RPC request to create new keys in your validator's keystore. Navigate to Toolbox tab and select **RPC Calls** then select the **author > rotateKeys()** option and remember to save the output that you get back for a later step.

![image](assets/rotateKeys.png)

## Submitting the setKeys Transaction

You need to tell the chain your Session keys by signing and submitting an extrinsic. This is what associates your validator with your Controller account.

Go to **Staking > Account Actions**, and click "Session Key" on the bonding account you generated earlier. Enter the output from author_rotateKeys in the field and click "Set Session Key".

![image](assets/setSessionKey.png)

Submit this extrinsic and you are now ready to start validating.

## Validate

Go to **Staking > Account Actions**, and click "validate".

- The "reward commission percentage" is the commission percentage that you can declare against your validator's rewards. This is the rate that your validator will be commissioned with.

- You can also determine if you would like to receive nominations with the "allows new nominations" option.

![image](assets/validator.png)

Congratulations! If you have followed all of these steps, and been selected to be a part of the validator set.