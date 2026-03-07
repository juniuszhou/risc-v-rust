import "dotenv/config";
import { ethers } from "ethers";
import * as fs from "fs";
import * as path from "path";
import solc from "solc";
import { POLKADOT_HUB_TESTNET } from "./config";

function getContractAbi(): ethers.InterfaceAbi {
  const contractPath = path.join(__dirname, "..", "contracts", "Counter.sol");
  const source = fs.readFileSync(contractPath, "utf8");

  const input = {
    language: "Solidity",
    sources: {
      "Counter.sol": { content: source },
    },
    settings: {
      outputSelection: {
        "*": { "*": ["abi"] },
      },
    },
  };

  const output = JSON.parse(solc.compile(JSON.stringify(input)));
  return output.contracts["Counter.sol"].Counter.abi as ethers.InterfaceAbi;
}

async function main() {
  const privateKey = process.env.PRIVATE_KEY;
  const contractAddress = process.env.CONTRACT_ADDRESS;

  if (!privateKey) {
    console.error("Missing PRIVATE_KEY in .env");
    process.exit(1);
  }
  if (!contractAddress) {
    console.error(
      "Missing CONTRACT_ADDRESS. Pass it as env var: CONTRACT_ADDRESS=0x... npm run interact"
    );
    process.exit(1);
  }

  const abi = getContractAbi();
  const provider = new ethers.JsonRpcProvider(POLKADOT_HUB_TESTNET.rpcUrl);
  const wallet = new ethers.Wallet(privateKey, provider);
  const contract = new ethers.Contract(contractAddress, abi, wallet);

  // Read current count
  const count = await contract.count();
  console.log("Current count:", count.toString());

  // Increment
  console.log("Incrementing...");
  const tx = await contract.increment();
  await tx.wait();
  console.log("Transaction hash:", tx.hash);

  // Read updated count
  const newCount = await contract.count();
  console.log("New count:", newCount.toString());
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
