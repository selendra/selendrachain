#!/bin/bash
db=${1:-all}

if [[ "$OSTYPE" == "linux-gnu" ]]; then
  echo "Clearing local data from home dir: $HOME/.local/share/indracore"
	if [[ "$db" == "staging" ]]; then
		rm -rf ~/.local/share/indracore/chains/staging_testnet/db/
	elif [[ "$db" == "dev" ]]; then
		rm -rf ~/.local/share/indracore/chains/dev/db/
		rm -rf ~/.local/share/indracore/chains/development/db/
	elif [[ "$db" == "indracore" ]]; then
		rm -rf ~/.local/share/indracore/chains/indracore/db/
		rm -rf ~/.local/share/indracore/chains/indracore_testnet/db/
	else
		rm -rf ~/.local/share/indracore/chains/dev/db/
		rm -rf ~/.local/share/indracore/chains/development/db/
		rm -rf ~/.local/share/indracore/chains/indracore/db/
		rm -rf ~/.local/share/indracore/chains/indracore_testnet/db/
		rm -rf ~/.local/share/indracore/chains/staging_testnet/db/
		rm -rf ~/.local/share/indracore/chains/local_testnet/db/
        rm -rf ~/.local/share/indracore/chains/$db/db/
	fi
elif [[ "$OSTYPE" == "darwin"* ]]; then
  echo "Clearing local data from home dir: $HOME/Library/Application Support/indracore"
	if [[ "$db" == "staging" ]]; then
		rm -rf ~/Library/Application\ Support/indracore/chains/staging_testnet/db/
	elif [[ "$db" == "dev" ]]; then
		rm -rf ~/Library/Application\ Support/indracore/chains/dev/db/
		rm -rf ~/Library/Application\ Support/indracore/chains/development/db/
	elif [[ "$db" == "indracore" ]]; then
		rm -rf ~/Library/Application\ Support/indracore/chains/indracore/db/
		rm -rf ~/Library/Application\ Support/indracore/chains/indracore_testnet/db/
	else
		rm -rf ~/Library/Application\ Support/indracore/chains/dev/db/
		rm -rf ~/Library/Application\ Support/indracore/chains/development/db/
		rm -rf ~/Library/Application\ Support/indracore/chains/indracore/db/
		rm -rf ~/Library/Application\ Support/indracore/chains/indracore_testnet/db/
		rm -rf ~/Library/Application\ Support/indracore/chains/staging_testnet/db/
		rm -rf ~/Library/Application\ Support/indracore/chains/local_testnet/db/
        rm -rf ~/Library/Application\ Support/indracore/chains/$db/db/
	fi
else
  echo "Clearing local data from home dir: $HOME/.local/share/indracore"
	if [[ "$db" == "staging" ]]; then
		rm -rf ~/.local/share/indracore/chains/staging_testnet/db/
	elif [[ "$db" == "dev" ]]; then
		rm -rf ~/.local/share/indracore/chains/dev/db/
		rm -rf ~/.local/share/indracore/chains/development/db/
	elif [[ "$db" == "indracore" ]]; then
		rm -rf ~/.local/share/indracore/chains/indracore/db/
		rm -rf ~/.local/share/indracore/chains/indracore_testnet/db/
	else
		rm -rf ~/.local/share/indracore/chains/dev/db/
		rm -rf ~/.local/share/indracore/chains/development/db/
		rm -rf ~/.local/share/indracore/chains/indracore/db/
		rm -rf ~/.local/share/indracore/chains/indracore_testnet/db/
		rm -rf ~/.local/share/indracore/chains/staging_testnet/db/
		rm -rf ~/.local/share/indracore/chains/local_testnet/db/
        rm -rf ~/.local/share/indracore/chains/$db/db/
	fi
fi

echo "Deleted $db databases"