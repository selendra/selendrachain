const API = require("@polkadot/api")
const Web3 = require("web3")
const selendraTypes = require('./node-types')

const web3 = new Web3("http://localhost:9933");

const PUB_KEY = "0x8C3b33C8cA283e26144e61426326ADc0514D0Fb5"  // Ethereum public key
const PRIV_KEY = "70cdc2290152ee0afc901d1fd7b6088f43612bb5bf7eb2a12860f3603e52b161" // Ethereum private key
const SELENDRS_SEEDS = "oppose vintage subject history pilot burden flat wolf pioneer luxury toe coast" // Mnemonic

async function run() {
    const wsProvider = new API.WsProvider('ws://127.0.0.1:9944');
    const api = await API.ApiPromise.create({
        provider: wsProvider,
        types: selendraTypes
    });
    const keyring = new API.Keyring({ type: 'sr25519' });
    const pioneer = keyring.addFromUri(SELENDRS_SEEDS);
    let nonce = await api.rpc.system.accountNextIndex(pioneer.address);
    web3.eth.accounts.wallet.add(PRIV_KEY);
    let signature = await web3.eth.sign(`Selendra evm:${web3.utils.bytesToHex(pioneer.publicKey).slice(2)}`, PUB_KEY);

    await api.tx.evmAccounts
        .claimAccount(PUB_KEY, web3.utils.hexToBytes(signature))
        .signAndSend(pioneer, {
            nonce,
        }, ({ events = [], status }) => {
            if (status.isFinalized) {
                console.log(`${pioneer.address} has bound with EVM address: ${PUB_KEY}`)
            }
        });
}

run()