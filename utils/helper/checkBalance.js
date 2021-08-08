const { ArgumentParser } = require('argparse');
const { ethers} = require('ethers');
const version  = require('./package.json');
 
const parser = new ArgumentParser({
  description: 'Selendra check balance with ether js'
});
 
parser.add_argument('-v', '--version', { action: 'version', version });
parser.add_argument(
    '-r', '--rpc',
    {
        default: "http://localhost:9933",
        help: 'Selendra rpc ,default is http://localhost:9933'
    }
);
parser.add_argument(
   '-a', '--address',
   {
       help: 'ethereum account addreess'
   }
);

const providerRPC = {
   development: {
      name: 'selendra',
      rpc: parser.parse_args().rpc,
      chainId: 2000,
   },
};

const provider = new ethers.providers.StaticJsonRpcProvider(
   providerRPC.development.rpc,
   {
      chainId: providerRPC.development.chainId,
      name: providerRPC.development.name,
   }
);

const balances = async () => {
   const balanceFrom = ethers.utils.formatEther(
      await provider.getBalance(parser.parse_args().address)
   );
   console.log(`The balance of ${parser.parse_args().address} is: ${balanceFrom} SELS`);
   process.exit(1);
};

balances();
