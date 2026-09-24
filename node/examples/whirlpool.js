// Query recent Whirlpool instruction calls and their parent transactions.
const { SolanaClient } = require("..");

const WHIRLPOOL = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";

async function main() {
  const client = new SolanaClient({
    url: process.env.HYPERSYNC_URL ?? "https://solana.hypersync.xyz",
    apiToken: process.env.HYPERSYNC_API_TOKEN,
  });

  const height = await client.getHeight();
  const fromSlot = Math.max(0, height - 100);

  const resp = await client.get({
    fromSlot,
    instructionCalls: [
      {
        executingAccount: [WHIRLPOOL],
      },
    ],
    fieldSelection: {
      instructionCall: ["slot", "transaction_index", "executing_account", "data"],
      transaction: ["slot", "transaction_index", "fee_payer", "success"],
    },
  });

  const ixs = resp.tables.instruction_calls ?? [];
  const txs = resp.tables.transactions ?? [];
  console.log(`scanned slots ${fromSlot} -> ${resp.nextSlot}`);
  console.log(`instruction calls: ${ixs.length}`);
  console.log(`transactions: ${txs.length}`);
  if (ixs.length) {
    console.log("first instruction call:", ixs[0]);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
