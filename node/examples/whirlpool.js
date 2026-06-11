// Query recent Whirlpool instructions and their parent transactions.
const { SolanaClient } = require("..");

const WHIRLPOOL = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";

async function main() {
  const client = new SolanaClient({
    url: process.env.HYPERSYNC_URL ?? "https://solana.hypersync.xyz",
    bearerToken: process.env.HYPERSYNC_BEARER_TOKEN,
  });

  const height = await client.getHeight();
  const fromSlot = Math.max(0, height - 100);

  const resp = await client.query({
    fromSlot,
    instructions: [
      {
        programId: [WHIRLPOOL],
      },
    ],
    fieldSelection: {
      instruction: ["slot", "transaction_index", "program_id", "data"],
      transaction: ["slot", "transaction_index", "fee_payer", "success"],
    },
  });

  const ixs = resp.tables.instructions ?? [];
  const txs = resp.tables.transactions ?? [];
  console.log(`scanned slots ${fromSlot} -> ${resp.nextSlot}`);
  console.log(`instructions: ${ixs.length}`);
  console.log(`transactions: ${txs.length}`);
  if (ixs.length) {
    console.log("first instruction:", ixs[0]);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
