## Turnturnturn, an Op Return message util

This is a small CLI to embed messages in Bitcoin Op Return fields. It creates a
local ephemeral address, watches the chain for funding, and sends a special
transaction containing your message.

This software is ALPHA QUALITY. It has been tested on testnet. Please do not
send any amount of money you are unwilling to lose. It has NO WARRANTY OF ANY
KIND.

### Flow

0. Use `$ cargo run -- --help` to see the help messages
1. Specify the message, fee, and optionally a change address:
    - Your message may be between 12 and 74 characters.
    - `$ cargo run -- -m "when true simplicity is gained" -f 10000`
2. The tool will print an address to fund, and then keep running
3. Send sats to that address
4. The tool will build and broadcast the transaction, and send funds to the
  change address.
5. After that it will stop.

If you close the tool (using ctrl+c), your in progress message and key are
saved. Simply re-run the tool and it will detect any funds you've sent to the
address.

To run on testnet, adjust the commands above to specify
`cargo run --no-default-features --features=testnet --`.

If you encounter any problems, post an issue. The ephemeral key is stored in
the JSON files. So DO NOT share or delete those.
