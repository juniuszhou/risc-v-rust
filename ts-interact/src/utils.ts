import * as fs from "fs";
import * as path from "path";
import { blake2b } from "@noble/hashes/blake2b";

const UNISWAP_ROOT = path.join(__dirname, "..", "..", "uniswap-rust");

export function getFactoryAbi() {
  const abiPath = path.join(UNISWAP_ROOT, "abis", "uniswap-v2-factory.json");
  return JSON.parse(fs.readFileSync(abiPath, "utf8"));
}

export function getPairAbi() {
  try {
    const abiPath = path.join(UNISWAP_ROOT, "abis", "uniswap-v2-pair.json");
    if (fs.existsSync(abiPath)) {
      return JSON.parse(fs.readFileSync(abiPath, "utf8"));
    }
  } catch {
    /* use fallback */
  }
  // Fallback inline ABI - matches IUniswapV2Pair interface
  return [
    { constant: true, inputs: [], name: "factory", outputs: [{ type: "address" }], type: "function" },
    { constant: true, inputs: [], name: "token0", outputs: [{ type: "address" }], type: "function" },
    { constant: true, inputs: [], name: "token1", outputs: [{ type: "address" }], type: "function" },
    {
      constant: true,
      inputs: [],
      name: "getReserves",
      outputs: [{ type: "uint112" }, { type: "uint112" }, { type: "uint32" }],
      type: "function",
    },
    { constant: true, inputs: [], name: "totalSupply", outputs: [{ type: "uint256" }], type: "function" },
    {
      constant: true,
      inputs: [{ name: "owner", type: "address" }],
      name: "balanceOf",
      outputs: [{ type: "uint256" }],
      type: "function",
    },
    {
      constant: false,
      inputs: [{ name: "token0_", type: "address" }, { name: "token1_", type: "address" }],
      name: "initialize",
      outputs: [],
      type: "function",
    },
    {
      constant: false,
      inputs: [{ name: "to", type: "address" }],
      name: "mint",
      outputs: [{ type: "uint256" }],
      type: "function",
    },
    {
      constant: false,
      inputs: [{ name: "to", type: "address" }],
      name: "burn",
      outputs: [{ type: "uint256" }, { type: "uint256" }],
      type: "function",
    },
    {
      constant: false,
      inputs: [
        { name: "amount0Out", type: "uint256" },
        { name: "amount1Out", type: "uint256" },
        { name: "to", type: "address" },
        { name: "data", type: "bytes" },
      ],
      name: "swap",
      outputs: [],
      type: "function",
    },
    { constant: false, inputs: [{ name: "to", type: "address" }], name: "skim", outputs: [], type: "function" },
    { constant: false, inputs: [], name: "sync", outputs: [], type: "function" },
    { constant: true, inputs: [], name: "MINIMUM_LIQUIDITY", outputs: [{ type: "uint256" }], type: "function" },
  ];
}

export function getFactoryBytecode(): Buffer {
  const bytePath = path.join(UNISWAP_ROOT, "uniswap-v2-factory.polkavm");
  return fs.readFileSync(bytePath);
}

export function getPairBytecode(): Buffer {
  const bytePath = path.join(UNISWAP_ROOT, "uniswap-v2-pair.polkavm");
  return fs.readFileSync(bytePath);
}

/** Blake2b-256 hash used by Substrate/pallet-revive for contract code */
export function codeHash(code: Uint8Array): Uint8Array {
  return blake2b(code, { dkLen: 32 });
}

/** Encode 20-byte address to 32 bytes (left-padded) for ABI encoding */
export function encodeAddress(addr: string | Uint8Array): Uint8Array {
  const bytes =
    typeof addr === "string"
      ? Buffer.from(addr.startsWith("0x") ? addr.slice(2) : addr, "hex")
      : addr;
  const buf = Buffer.alloc(32, 0);
  const src = bytes.length === 20 ? bytes : bytes.slice(-20);
  Buffer.from(src).copy(buf, 12);
  return new Uint8Array(buf);
}

/** Derive 20-byte EVM-style address from 32-byte public key (first 20 bytes) */
export function pubkeyToAccountId20(publicKey: Uint8Array): Uint8Array {
  return publicKey.slice(0, 20);
}

/** Encode factory constructor: (feeToSetter: address, pairCodeHash: bytes32) - 64 bytes */
export function encodeFactoryConstructor(feeToSetter: string, pairCodeHash: Uint8Array): Uint8Array {
  const out = new Uint8Array(64);
  const feeSetterPadded = encodeAddress(feeToSetter);
  out.set(feeSetterPadded, 0);
  out.set(pairCodeHash, 32);
  return out;
}
