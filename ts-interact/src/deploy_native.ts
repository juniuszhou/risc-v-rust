import "dotenv/config";
import { POLKADOT_HUB_TESTNET } from "./config";
import { hub } from "@polkadot-api/descriptors"
import { createClient, } from "polkadot-api"
import { getWsProvider } from "polkadot-api/ws-provider"
import { withPolkadotSdkCompat } from "polkadot-api/polkadot-sdk-compat"
import { getPolkadotSigner } from "polkadot-api/signer"
import { sr25519CreateDerive } from "@polkadot-labs/hdkd"
import {
  DEV_PHRASE,
  entropyToMiniSecret,
  mnemonicToEntropy,
  ss58Address, ss58Decode
} from "@polkadot-labs/hdkd-helpers"

async function main() {
  const seed = process.env.SEED;
  const client = await createClient(withPolkadotSdkCompat(getWsProvider("wss://dot-rpc.stakeworld.io")),);
  const api = await client.getTypedApi(hub);

  const miniSecret = entropyToMiniSecret(mnemonicToEntropy(seed ?? ""))
  const derive = sr25519CreateDerive(miniSecret)
  const hdkdKeyPair = derive("//Alice") // or `//Bob`, `//Charlie`, etc

  const polkadotSigner = getPolkadotSigner(
    hdkdKeyPair.publicKey,
    "Sr25519",
    hdkdKeyPair.sign,
  )
  const ss58 = ss58Address(hdkdKeyPair.publicKey, 42);
  console.log(ss58);

  const query = await api.query.System.Account.getValue(ss58);
  console.log(query.data.free);

  const tx = await api.tx.Revive.instantiate_with_code({
    data: new Binary("0x00"),
    value: "0x00",
    weight_limit: new Weight("0x00"),
    admin: ss58,
  });
  console.log(tx);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
