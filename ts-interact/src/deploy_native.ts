import "dotenv/config";
import * as fs from "fs";
import * as path from "path";
import { hub } from "@polkadot-api/descriptors";
import { createClient, Binary, FixedSizeBinary } from "polkadot-api";
import { getWsProvider } from "polkadot-api/ws-provider";
import { withPolkadotSdkCompat } from "polkadot-api/polkadot-sdk-compat";
import { getPolkadotSigner } from "polkadot-api/signer";
import { sr25519CreateDerive } from "@polkadot-labs/hdkd";
import {
  entropyToMiniSecret,
  mnemonicToEntropy,
  ss58Address,
} from "@polkadot-labs/hdkd-helpers";
import { ethers } from "ethers";
import {
  getFactoryAbi,
  getFactoryBytecode,
  getPairBytecode,
  getPairAbi,
  codeHash,
  encodeFactoryConstructor,
  pubkeyToAccountId20,
} from "./utils";

// Load hub RPC from papi config
// const PAPI_JSON = JSON.parse(
//   fs.readFileSync(path.join(__dirname, "..", ".papi", "polkadot-api.json"), "utf8")
// );
const WS_URL = "wss://sys.turboflakes.io/asset-hub-paseo";

const WEIGHT_LIMIT = { ref_time: BigInt(500_000_000_000), proof_size: BigInt(64 * 1024) };
const STORAGE_DEPOSIT_LIMIT = BigInt(1000000000000000); // 0.001 DOT

async function main() {
  const seed = process.env.SEED;
  console.log("Seed:", seed);

  console.log("Connecting to", WS_URL);
  const client = await createClient(
    withPolkadotSdkCompat(getWsProvider(WS_URL))
  );
  const api = await client.getTypedApi(hub);

  const miniSecret = entropyToMiniSecret(mnemonicToEntropy(seed ?? ""));
  const derive = sr25519CreateDerive(miniSecret);
  const hdkdKeyPair = derive("//Alice");

  const polkadotSigner = getPolkadotSigner(
    hdkdKeyPair.publicKey,
    "Sr25519",
    hdkdKeyPair.sign
  );

  const ss58 = ss58Address(hdkdKeyPair.publicKey, 42);
  const accountId20 = pubkeyToAccountId20(hdkdKeyPair.publicKey);
  const feeToSetterHex = "0x" + Buffer.from(accountId20).toString("hex");

  console.log("Deployer SS58:", ss58);
  console.log("Deployer AccountId20:", feeToSetterHex);

  const query = await api.query.System.Account.getValue(ss58);
  const balance = query.data.free;
  console.log("Balance:", balance.toString());

  if (balance === BigInt(0)) {
    console.error("\nAccount has zero balance. Fund it from the testnet faucet:");
    console.error("  https://faucet.polkadot.io/ (select Asset Hub Paseo)");
    process.exit(1);
  }

  // Load bytecode
  const pairBytecode = getPairBytecode();
  const factoryBytecode = getFactoryBytecode();
  const pairCodeHashBytes = codeHash(new Uint8Array(pairBytecode));

  // const mapAccountTx = api.tx.Revive.map_account();
  // const mapAccountResult = await mapAccountTx.signAndSubmit(polkadotSigner);
  // if (!mapAccountResult.ok) {
  //   throw new Error("Map account failed: " + JSON.stringify(mapAccountResult.dispatchError));
  // }

  // Step 1: Upload pair code (instantiate pair with empty deploy to register code on chain)
  console.log("\n1. Uploading pair code...");
  const pairUploadTx = api.tx.Revive.instantiate_with_code({
    value: BigInt(0),
    weight_limit: WEIGHT_LIMIT,
    storage_deposit_limit: STORAGE_DEPOSIT_LIMIT,
    code: Binary.fromBytes(new Uint8Array(pairBytecode)),
    data: Binary.fromBytes(new Uint8Array(0)),
    salt: undefined,
  });

  const pairUploadResult = await pairUploadTx.signAndSubmit(polkadotSigner);
  if (!pairUploadResult.ok) {
    throw new Error("Pair upload failed: " + JSON.stringify(pairUploadResult.dispatchError));
  }
  console.log("Pair code uploaded, block:", pairUploadResult.block.hash);

  // Step 2: Deploy factory with constructor (feeToSetter, pairCodeHash)
  const factoryData = encodeFactoryConstructor(
    feeToSetterHex,
    pairCodeHashBytes
  );

  console.log("\n2. Deploying factory...");
  const factoryTx = api.tx.Revive.instantiate_with_code({
    value: BigInt(0),
    weight_limit: WEIGHT_LIMIT,
    storage_deposit_limit: STORAGE_DEPOSIT_LIMIT,
    code: Binary.fromBytes(new Uint8Array(factoryBytecode)),
    data: Binary.fromBytes(factoryData),
    salt: undefined,
  });

  const factoryResult = await factoryTx.signAndSubmit(polkadotSigner);
  if (!factoryResult.ok) {
    throw new Error("Factory deploy failed: " + JSON.stringify(factoryResult.dispatchError));
  }

  console.log("result:", factoryResult);

  // Extract factory address from Instantiated event
  const factoryAddress = extractContractAddressFromEvents(factoryResult);
  if (!factoryAddress) {
    throw new Error("Could not extract factory address from events");
  }
  console.log("Factory deployed at:", factoryAddress);

  // Step 3: Call factory view functions
  console.log("\n3. Calling factory functions...");
  await callContract(api, polkadotSigner, factoryAddress, getFactoryAbi(), [
    ["feeTo", []],
    ["feeToSetter", []],
    ["allPairsLength", []],
  ]);

  // Step 4: createPair requires two ERC20 token addresses.
  // For a full test we need ERC20 tokens. We'll use placeholder addresses
  // (createPair will instantiate the pair; mint/swap need real ERC20s).
  const tokenA = "0x0000000000000000000000000000000000000001";
  const tokenB = "0x0000000000000000000000000000000000000002";
  const [token0, token1] =
    tokenA.toLowerCase() < tokenB.toLowerCase()
      ? [tokenA, tokenB]
      : [tokenB, tokenA];

  console.log("\n4. Creating pair for tokens", token0, token1);
  const createPairData = new ethers.Interface(getFactoryAbi()).encodeFunctionData(
    "createPair",
    [tokenA, tokenB]
  );

  const createPairTx = api.tx.Revive.call({
    dest: hexToFixedBinary20(factoryAddress),
    value: BigInt(0),
    weight_limit: WEIGHT_LIMIT,
    storage_deposit_limit: STORAGE_DEPOSIT_LIMIT,
    data: Binary.fromBytes(hexToBytes(createPairData)),
  });

  const createPairResult = await createPairTx.signAndSubmit(polkadotSigner);
  if (!createPairResult.ok) {
    console.warn("createPair failed (expected if tokens are not ERC20):", createPairResult.dispatchError);
  } else {
    const pairAddress = extractReturnAddressFromEvents(createPairResult);
    if (pairAddress) {
      console.log("Pair created at:", pairAddress);

      // Step 5: Call pair view functions
      console.log("\n5. Calling pair functions...");
      await callContract(api, polkadotSigner, pairAddress, getPairAbi(), [
        ["factory", []],
        ["token0", []],
        ["token1", []],
        ["getReserves", []],
        ["totalSupply", []],
        ["MINIMUM_LIQUIDITY", []],
      ]);
    }
  }

  // Final factory state
  console.log("\n6. Final factory state:");
  await callContract(api, polkadotSigner, factoryAddress, getFactoryAbi(), [
    ["allPairsLength", []],
    ["getPair", [tokenA, tokenB]],
  ]);

  console.log("\nDone.");
}

function hexToBytes(hex: string): Uint8Array {
  const h = hex.startsWith("0x") ? hex.slice(2) : hex;
  return new Uint8Array(Buffer.from(h, "hex"));
}

function hexToFixedBinary20(hex: string) {
  const h = hex.startsWith("0x") ? hex.slice(2) : hex;
  const buf = Buffer.from(h, "hex");
  if (buf.length !== 20) {
    throw new Error("Address must be 20 bytes, got " + buf.length);
  }
  return new FixedSizeBinary(new Uint8Array(buf));
}

async function callContract(
  api: any,
  signer: any,
  address: string,
  abi: any[],
  calls: [string, any[]][]
) {
  const iface = new ethers.Interface(abi);

  for (const [name, args] of calls) {
    try {
      const calldata = iface.encodeFunctionData(name, args);
      const tx = api.tx.Revive.call({
        dest: hexToFixedBinary20(address),
        value: BigInt(0),
        weight_limit: WEIGHT_LIMIT,
        storage_deposit_limit: STORAGE_DEPOSIT_LIMIT,
        data: Binary.fromBytes(hexToBytes(calldata)),
      });

      const result = await tx.signAndSubmit(signer);
      if (!result.ok) {
        throw new Error(JSON.stringify(result.dispatchError));
      }
      const output = extractCallReturnFromEvents(result);
      const fragment = iface.getFunction(name);
      let decoded: any;
      try {
        decoded = fragment
          ? iface.decodeFunctionResult(fragment, "0x" + Buffer.from(output).toString("hex"))
          : output;
      } catch {
        decoded = "0x" + Buffer.from(output).toString("hex");
      }
      console.log(`  ${name}():`, decoded);
    } catch (e) {
      console.warn(`  ${name}():`, (e as Error).message);
    }
  }
}

function extractContractAddressFromEvents(result: { events: any[] }): string | null {
  const events = result.events ?? [];
  for (const ev of events) {
    const e = ev?.event ?? ev;
    const instantiated = e?.Revive?.Instantiated ?? e?.Instantiated;
    if (instantiated) {
      const addr = instantiated.contract ?? instantiated;
      if (addr && typeof addr === "object" && "AccountId20" in addr) {
        return "0x" + Buffer.from(addr.AccountId20 as Uint8Array).toString("hex");
      }
      if (addr && typeof addr === "object" && Array.isArray(addr) && addr.length === 20) {
        return "0x" + Buffer.from(addr as unknown as Uint8Array).toString("hex");
      }
      if (addr && addr instanceof Uint8Array && addr.length === 20) {
        return "0x" + Buffer.from(addr).toString("hex");
      }
      if (typeof addr === "string" && addr.startsWith("0x")) return addr;
    }
  }
  return null;
}

function extractReturnAddressFromEvents(result: { events: any[] }): string | null {
  const output = extractCallReturnFromEvents(result);
  if (output && output.length >= 32) {
    return "0x" + Buffer.from(output.slice(12, 32)).toString("hex");
  }
  return null;
}

function extractCallReturnFromEvents(result: { events: any[] }): Uint8Array {
  const events = result?.events ?? [];
  for (const ev of events) {
    const e = ev?.event ?? ev;
    const emitted = e?.Revive?.ContractEmitted ?? e?.ContractEmitted;
    if (emitted) {
      const data = emitted.data ?? emitted;
      return data ? (typeof data === "string" ? hexToBytes(data) : new Uint8Array(data)) : new Uint8Array(0);
    }
  }
  return new Uint8Array(0);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
