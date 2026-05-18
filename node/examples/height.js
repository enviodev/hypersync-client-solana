const { SolanaClient } = require("..");

async function main() {
  const client = new SolanaClient({
    url: process.env.HYPERSYNC_URL ?? "https://solana.hypersync.xyz",
    bearerToken: process.env.HYPERSYNC_BEARER_TOKEN,
  });

  const height = await client.getHeight();
  console.log("current Solana slot:", height);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
