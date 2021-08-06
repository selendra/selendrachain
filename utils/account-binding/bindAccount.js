const { ArgumentParser } = require('argparse');
const { ethers} = require('ethers');
const API = require("@polkadot/api")
const Web3 = require("web3")
const version  = require('./package.json');
const selendraTypes = require('./node-types')
 
const parser = new ArgumentParser({
  description: 'Selendra evm account binding. Use to bind substrate account to ethereum account'
});
 
parser.add_argument('-v', '--version', { action: 'version', version });
parser.add_argument(
    '-m', '--mnemonic', 
    {
        help: 'Account mnemonic, 12 or 24 words'
    },
);

parser.add_argument(
    '-u', '--url',
    {
        default: "ws://127.0.0.1:9944",
        help: 'Websocket provider ,default is http://localhost:9933'
    }
);

parser.add_argument(
    '-r', '--rpc',
    {
        default: "http://localhost:9933",
        help: 'Selendra rpc ,default is http://localhost:9933'
    }
);

async function run() {
    const wsProvider = new API.WsProvider(parser.parse_args().url);
    const web3 = new Web3(parser.parse_args().rpc);

    const api = await API.ApiPromise.create({
        provider: wsProvider,
        types: selendraTypes
    });

    const keyring = new API.Keyring({ type: 'sr25519' });
    const substrate_account = keyring.addFromUri(parser.parse_args().mnemonic);
    let wallet = ethers.Wallet.fromMnemonic(parser.parse_args().mnemonic);

    let nonce = await api.rpc.system.accountNextIndex(substrate_account.address);
    web3.eth.accounts.wallet.add(wallet.privateKey);
    let signature = await web3.eth.sign(`Selendra evm:${web3.utils.bytesToHex(substrate_account.publicKey).slice(2)}`, wallet.address);

    await api.tx.evmAccounts
        .claimAccount(wallet.address, web3.utils.hexToBytes(signature))
        .signAndSend(substrate_account, {
            nonce,
        }, ({ events = [], status }) => {
            if (status.isFinalized) {
                console.log(`${substrate_account.address} has bound with EVM address: ${wallet.address}`)
                process.exit(1)
            }
        });
}

run()