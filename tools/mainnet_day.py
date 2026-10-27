#!/usr/bin/env python3
"""Build a netting input from one UTC day of real Stellar mainnet payments.

Participants are institutions, not accounts: every account stellar.expert's
public directory tags as an exchange, anchor, custodian or wallet is grouped
under its organization (its home domain, else its name). The day's successful
USDC payments (Circle's issuer only) between accounts of two *different*
institutions become obligations, one per payment operation, read from
Horizon. Each obligation keeps its operation id, so every row can be checked
on a block explorer.

    python3 tools/mainnet_day.py 2026-10-21 > data/mainnet-2026-10-21.csv

Output is the engine's CSV (`id,debtor,creditor,asset,amount`; amounts in
stroops, USDC's 7-decimal unit). A summary goes to stderr. Stdlib only.
"""

import csv
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timedelta, timezone

HORIZON = "https://horizon.stellar.org"
DIRECTORY = "https://api.stellar.expert/explorer/directory"
USDC_ISSUER = "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"
TAGS = ["exchange", "anchor", "custodian", "wallet"]
MAX_PAGES = 60  # per account, 200 operations each; busy accounts are capped and reported
UA = {"User-Agent": "setoff-engine mainnet_day (github.com/SetOff-Org/setoff-engine)"}


def get(url, tries=6):
    for attempt in range(tries):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=60) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return None
            if e.code == 429 or e.code >= 500:
                time.sleep(min(60, 2 ** attempt * 3))
                continue
            raise
        except (urllib.error.URLError, TimeoutError):
            time.sleep(2 ** attempt)
    raise RuntimeError(f"giving up on {url}")


def directory():
    """address -> institution, for classic (G…) accounts in the tagged directory."""
    out = {}
    for tag in TAGS:
        url = f"{DIRECTORY}?tag[]={tag}&limit=200"
        while url:
            page = get(url)
            records = page["_embedded"]["records"]
            for r in records:
                if r["address"].startswith("G"):
                    out[r["address"]] = (r.get("domain") or r.get("name") or r["address"]).lower()
            nxt = page["_links"]["next"]["href"]
            url = f"https://api.stellar.expert{nxt}" if records else None
    return out


def is_usdc_payment(op):
    if not op.get("transaction_successful"):
        return False
    if op["type"] == "payment":
        return op.get("asset_code") == "USDC" and op.get("asset_issuer") == USDC_ISSUER
    if op["type"] in ("path_payment_strict_send", "path_payment_strict_receive"):
        # Both legs USDC: a plain transfer routed as a path payment.
        return (op.get("asset_code") == "USDC" and op.get("asset_issuer") == USDC_ISSUER
                and op.get("source_asset_code") == "USDC" and op.get("source_asset_issuer") == USDC_ISSUER)
    return False


def stroops(amount):
    whole, _, frac = amount.partition(".")
    return int(whole) * 10_000_000 + int((frac + "0000000")[:7])


def day_payments(account, start, end):
    """The account's operations in [start, end), newest first, and whether the page cap was hit."""
    url = f"{HORIZON}/accounts/{account}/payments?order=desc&limit=200"
    ops, pages = [], 0
    while url and pages < MAX_PAGES:
        page = get(url)
        if page is None:
            return ops, False
        records = page["_embedded"]["records"]
        pages += 1
        for op in records:
            t = datetime.fromisoformat(op["created_at"].replace("Z", "+00:00"))
            if t < start:
                return ops, False
            if t < end:
                ops.append(op)
        if len(records) < 200:
            return ops, False
        url = page["_links"]["next"]["href"]
    return ops, url is not None and pages >= MAX_PAGES


def main():
    day = datetime.strptime(sys.argv[1], "%Y-%m-%d").replace(tzinfo=timezone.utc)
    start, end = day, day + timedelta(days=1)
    who = directory()
    print(f"directory: {len(who)} accounts, {len(set(who.values()))} institutions", file=sys.stderr)

    seen, rows, capped = set(), [], []
    for i, account in enumerate(sorted(who)):
        ops, hit_cap = day_payments(account, start, end)
        if hit_cap:
            capped.append(f"{who[account]} ({account[:6]}…)")
        for op in ops:
            frm, to = op.get("from"), op.get("to")
            if op["id"] in seen or not is_usdc_payment(op) or frm not in who or to not in who:
                continue
            if who[frm] == who[to]:
                continue  # an institution moving funds between its own accounts
            seen.add(op["id"])
            rows.append((op["id"], who[frm], who[to], "USDC", stroops(op["amount"]), op["transaction_hash"]))
        if (i + 1) % 25 == 0:
            print(f"  {i + 1}/{len(who)} accounts, {len(rows)} inter-institution payments", file=sys.stderr)

    rows.sort(key=lambda r: int(r[0]))
    w = csv.writer(sys.stdout, lineterminator="\n")
    w.writerow(["id", "debtor", "creditor", "asset", "amount"])
    for r in rows:
        w.writerow(r[:5])
    print(f"{sys.argv[1]}: {len(rows)} USDC payments between {len({r[1] for r in rows} | {r[2] for r in rows})} institutions",
          file=sys.stderr)
    if capped:
        print(f"capped at {MAX_PAGES * 200} operations (more payments exist): {', '.join(capped)}", file=sys.stderr)


if __name__ == "__main__":
    main()
