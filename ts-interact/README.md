# Polkadot Hub Testnet - Contract Deploy & Interact

TypeScript scripts to deploy and interact with smart contracts on [Polkadot Hub testnet](https://polkadot.com/platform/hub).

## Setup

```bash
npm install
cp .env.example .env
# Edit .env and add your PRIVATE_KEY
```

Get test tokens from the [Polkadot faucet](https://faucet.polkadot.io/).

## Usage

**Deploy the Counter contract:**

```bash
npm run deploy
```

Or with ts-node directly:

```bash
ts-node src/deploy.ts
```

**Interact with a deployed contract:**

```bash
CONTRACT_ADDRESS=0x... npm run interact
```

Or with ts-node:

```bash
CONTRACT_ADDRESS=0xYourDeployedAddress ts-node src/interact.ts
```

The interact script reads the current count, increments it, and prints the new count.

## Environment

| Variable         | Description                              |
|-----------------|------------------------------------------|
| `PRIVATE_KEY`   | Wallet private key (with `0x` prefix)    |
| `CONTRACT_ADDRESS` | Deployed contract address (for interact) |

## Network

- **RPC:** https://services.polkadothub-rpc.com/testnet
- **Chain ID:** 420420417
