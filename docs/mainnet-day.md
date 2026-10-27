# One real mainnet day, netted

What would SetOff have changed for institutions settling USDC with each other
on Stellar? This runs the engine on every USDC payment made between known
institutions on one real day, 2026-10-21 (UTC).

## Result

336 payments between 10 institutions, 55,532,924.24 USDC in total.

| Settlement | USDC moved | Saved | Transfers |
|---|---:|---:|---:|
| Gross (every payment on its own, as happened) | 55,532,924.24 | | 336 |
| Bilateral (each pair settles its difference) | 53,505,367.08 | 3.7% | 13 |
| Multilateral (SetOff: each settles its net position) | 53,355,736.73 | 3.9% | **9** |

**336 payments become 9 transfers, 37 times fewer.** The value saved is
small on this day because the flows were largely one-way: most of them
were exchanges and Circle (`centre.io`) moving USDC in a single direction,
which netting cannot offset. The saving is in operations: nine transactions
to sign, fee, submit and reconcile instead of 336, and nine places a
settlement can fail instead of 336.

The largest net debtors, which is what each would actually have paid:

| Institution | Net USDC owed |
|---|---:|
| binance.com | 48,919,022.28 |
| kraken.com | 4,340,681.00 |
| changelly.com | 60,000.00 |
| bitstamp.net | 26,028.45 |
| hitbtc.com | 10,005.00 |

The more two-way a corridor is, the more value netting saves too. The
[simulator](https://setoff-org.github.io/setoff-engine/) loads this day with
**A real mainnet day**, and takes your own CSV with **Import CSV**.

## How it was built

[`tools/mainnet_day.py`](../tools/mainnet_day.py) (Python standard library only):

1. **Institutions.** Every classic account that stellar.expert's public
   [directory](https://stellar.expert/directory) tags as an exchange, anchor,
   custodian or wallet: 480 accounts, grouped by organization (home domain,
   else name) into 229 institutions. An institution's accounts are one
   participant, so moving funds between its own accounts is not an
   obligation.
2. **Payments.** For each account, Horizon's payments for that UTC day.
   Kept: successful `payment` operations in USDC issued by Circle
   (`GA5Z…KZVN`), and path payments with USDC on both legs, whose two ends
   belong to *different* institutions. Each operation is counted once,
   though it is seen from both ends.
3. **Obligations.** One row per payment in the engine's CSV format, with the
   operation id as the row id, so every row can be looked up on a block
   explorer (`https://stellar.expert/explorer/public/op/<id>`). Amounts are
   stroops (10⁻⁷ USDC), exact.

The file is [`data/mainnet-2026-10-21.csv`](../data/mainnet-2026-10-21.csv).

## Limits

- **A lower bound.** Three very busy accounts (binance.com, ultracapital.xyz
  and aps.money) have more payments that day than the script reads per
  account (12,000 operations). Their payments with the other institutions are
  still captured from the other end, but payments between two of those three
  can be missing.
- **The directory is not complete.** Institutions it does not list, and
  their payments, are not included.
- **Observed payments, not obligations.** On chain, these payments settled
  one by one. Treating a day's payments as one netting window shows what
  settling the net would have changed, not what the institutions owed each
  other at any moment.

## Reproduce

```sh
python3 tools/mainnet_day.py 2026-10-21 > data/mainnet-2026-10-21.csv
cargo run --example mainnet_day --features csv -- data/mainnet-2026-10-21.csv
```

Any other day works the same way. Fetching takes a while within Horizon's
rate limits.
