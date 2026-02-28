# nw — Net Worth Tracker CLI

A minimal command-line tool for tracking personal net worth over time.
Data is entered manually. No accounts connected. No external API calls. Single binary.

## Philosophy

Most net worth tools require linking bank accounts, deal in real-time data, or live in a browser.
This tool does none of that. It is a plain ledger: you enter numbers, it stores them, it shows you trends.

- **Long-term focused** — designed for monthly or quarterly check-ins, not daily tracking
- **Manual entry only** — you decide what counts and when
- **Multi-currency** — assets in any currency, converted to USD at display time using per-snapshot rates
- **Snapshot-based** — each entry is a point-in-time record, not a live feed
- **USD as base** — all display and history is in USD; per-snapshot rates handle the conversion
- **Plain JSON storage** — `~/.config/nw-tracker/portfolio.json`, easy to back up and version with git

## Installation

```sh
cargo install --path .
```

Requires [Rust](https://rustup.rs) stable. The binary is named `nw`.

## Storage

All data lives in `~/.config/nw-tracker/portfolio.json` (respects `$XDG_CONFIG_HOME`).
The file is never modified in place — writes go to a `.json.tmp` sibling that is atomically renamed into place.

**Recommended backup strategy:** keep `portfolio.json` in a private git repository.

## Concepts

**Asset** — a thing you own with a stable identity: a brokerage account, a savings account, a crypto wallet, a real-estate position, etc. Each asset has an ID, a display name, a category (free-form string), and a currency.

**Snapshot** — a dated record of asset values. You add one whenever you want to capture a moment. Each snapshot stores:
- exchange rates for every non-USD currency in your asset list (as "1 USD = N foreign units")
- a value for each asset you want to include (assets can be omitted if the value is unknown)

**Category** — a free-form grouping string (e.g. `etf`, `crypto`, `bank`, `cash`, `real-estate`). No fixed list.

## Commands

### Asset management

```sh
# Add a new asset
nw asset add --id <id> --name <name> --category <category> --currency <currency>

# Edit an existing asset (all flags optional)
nw asset edit --id <id> [--name <name>] [--category <category>] [--currency <currency>]

# Remove an asset (prompts for confirmation if it appears in snapshots)
nw asset remove --id <id>

# List all assets
nw asset list
```

**Examples:**
```sh
nw asset add --id vti-brokerage  --name "VTI"             --category etf  --currency USD
nw asset add --id savings-chf    --name "Savings Account" --category bank --currency CHF
nw asset add --id eur-cash       --name "EUR Cash"        --category cash --currency EUR
nw asset edit --id vti-brokerage --name "VTI (Brokerage)"
nw asset remove --id eur-cash
```

---

### Snapshot management

```sh
# Record a new snapshot for a date (interactive prompts follow)
nw snapshot add --date <YYYY-MM-DD>

# Edit an existing snapshot (prompts pre-filled with existing values)
nw snapshot edit --date <YYYY-MM-DD>

# List all snapshots
nw snapshot list
```

`snapshot add` and `snapshot edit` are interactive:
1. For each non-USD currency in your asset list, enter the exchange rate as "1 USD = N units" (e.g. for EUR: if 1 USD buys 0.92 EUR, enter `0.92`)
2. For each asset, enter its current value in its native currency — press Enter to omit

Snapshots are always stored in ascending date order regardless of insertion order, so backfilling old dates is safe.

**Example:**
```sh
nw snapshot add --date 2025-06-01
# --- Exchange Rates ---
# CHF rate (1 USD = ? CHF): 0.90
# EUR rate (1 USD = ? EUR): 0.92
# --- Asset Values (press Enter to omit) ---
# VTI (ETF, USD): 12500
# Savings Account (BANK, CHF): 9000
# EUR Cash (CASH, EUR): 800
```

---

### Display

```sh
# Show latest snapshot
nw show

# Show a specific past snapshot
nw show --date <YYYY-MM-DD>

# Filter display to a single category
nw show --category <category>

# Show net worth history over a time range
nw history --range <1M|6M|1Y|5Y|ALL>
```

**`nw show` output:**
```
CURRENT NET WORTH — 2025-06-01

ETF
  Name         Currency   Value (native)   Value (USD)
  VTI          USD            12,500.00    12,500.00
  Subtotal                                12,500.00

BANK
  Savings      CHF              9,000.00    10,000.00
  Subtotal                                10,000.00

CASH
  EUR Cash     EUR                800.00       869.57
  Subtotal                                   869.57

TOTAL  23,369.57

ALLOCATION
  ETF           53.5%
  BANK          42.8%
  CASH           3.7%
```

**`nw history` output:**
```
NET WORTH HISTORY — 1Y

Date         Total (USD)   Change (USD)   Change %
2024-06-01    42,300.00    —              —
2024-09-01    45,100.00    +2,800.00      +6.62%
2024-12-01    43,800.00    -1,300.00      -2.88%
2025-03-01    48,200.00    +4,400.00      +10.05%
2025-06-01    51,400.00    +3,200.00      +6.64%
```

History ranges: `1M` (1 month), `6M` (6 months), `1Y` (1 year), `5Y` (5 years), `ALL`.

---

## Data format

`portfolio.json` is human-readable and straightforward to edit by hand if needed:

```json
{
  "assets": [
    {
      "id": "vti-brokerage",
      "name": "VTI",
      "category": "etf",
      "currency": "USD"
    },
    {
      "id": "savings-chf",
      "name": "Savings Account",
      "category": "bank",
      "currency": "CHF"
    }
  ],
  "snapshots": [
    {
      "date": "2025-06-01",
      "rates": {
        "CHF": 0.90,
        "EUR": 0.92
      },
      "entries": [
        { "asset_id": "vti-brokerage", "value": 12500.0 },
        { "asset_id": "savings-chf",   "value": 9000.0 }
      ]
    }
  ]
}
```

**Rules:**
- `rates` stores non-USD currencies only, as "1 USD = N foreign units". USD is always 1.0 by definition.
- `rates` only contains currencies that appear in the asset list.
- `entries` may omit assets — partial snapshots are valid.
- `value` is always in the asset's native currency. Conversion to USD happens at display time.
- `category` is lowercase; `currency` is uppercase ISO 4217 code.
- Snapshots are always sorted ascending by date.

## License

MIT
