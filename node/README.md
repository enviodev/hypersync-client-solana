# @envio-dev/hypersync-client-solana

Node.js bindings for the Solana HyperSync client, built with [napi-rs](https://napi.rs/).

## Install

```bash
yarn add @envio-dev/hypersync-client-solana
# or
npm install @envio-dev/hypersync-client-solana
```

## Quick start

```js
const { SolanaClient } = require("@envio-dev/hypersync-client-solana");

const client = new SolanaClient({
  url: "https://solana.hypersync.xyz",
  // bearerToken: "<your-token>",
});

(async () => {
  const height = await client.getHeight();
  console.log("current slot:", height);

  const resp = await client.query({
    fromSlot: height - 100,
    instructions: [
      {
        programId: ["whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"],
        includeTransaction: true,
      },
    ],
    fieldSelection: {
      instruction: ["slot", "transaction_index", "program_id", "data"],
      transaction: ["slot", "transaction_index", "fee_payer"],
    },
  });

  console.log("next slot:", resp.nextSlot);
  console.log("instructions:", resp.tables.instructions?.length ?? 0);
  console.log("transactions:", resp.tables.transactions?.length ?? 0);
})();
```

## Build from source

```bash
yarn install
yarn build      # release build
yarn build:debug
```

This produces `index.js`, `index.d.ts`, and a platform-specific `.node` file in
this directory.

## API surface (v0.1)

- `new SolanaClient(config)`
- `client.getHeight(): Promise<number>`
- `client.query(query): Promise<QueryResponse>`

Field names on the JS objects are `camelCase`. Field names inside returned row
objects are `snake_case` (they mirror the on-the-wire schema).
