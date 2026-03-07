import "dotenv/config";
import { ethers } from "ethers";
import * as fs from "fs";
import * as path from "path";
import "dotenv/config";
import { POLKADOT_HUB_TESTNET } from "./config";

async function getAbiBytecode() {
  const abiPath = path.join(__dirname, "../..", "fibonacci-rust", "fibonacci.json");
  const abi = JSON.parse(fs.readFileSync(abiPath, "utf8"));
  const bytePath = path.join(__dirname, "../..", "fibonacci-rust", "fibonacci.polkavm");
  const bytecode = fs.readFileSync(bytePath, "hex");
  return { abi, bytecode };
}
async function main() {
  const privateKey = process.env.PRIVATE_KEY;
  if (!privateKey) {
    console.error(
      "Missing PRIVATE_KEY. Set it in .env or run: PRIVATE_KEY=0x... npm run deploy"
    );
    process.exit(1);
  }

  console.log("Compiling Counter contract...");
  const { abi, bytecode } = await getAbiBytecode();

  const provider = new ethers.JsonRpcProvider(POLKADOT_HUB_TESTNET.rpcUrl);
  const wallet = new ethers.Wallet(privateKey, provider);

  console.log("Deploying to Polkadot Hub testnet...");
  console.log("Deployer address:", wallet.address);

  const factory = new ethers.ContractFactory(abi, bytecode, wallet);
  const contract = await factory.deploy();

  await contract.waitForDeployment();
  const address = await contract.getAddress();

  console.log("Counter deployed at:", address);
  console.log("Save this address to interact with the contract:");
  console.log(`  CONTRACT_ADDRESS=${address} npm run interact`);
}

async function callContract() {
  const provider = new ethers.JsonRpcProvider(POLKADOT_HUB_TESTNET.rpcUrl);
  const { abi, } = await getAbiBytecode();
  const contractAddress = "0x3AaC58784ff35944AaE453dA9405271981B22d3d";
  const contract = new ethers.Contract(contractAddress, abi, provider);
  const result = await contract.fibonacci(20);
  console.log("Fibonacci result:", result);
}

callContract().catch((err) => {
  console.error(err);
  process.exit(1);
});
