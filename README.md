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

Requires [Rust](https://rustup.rs) 1.100+ (Edition 2024). The binary is named `nw`.

## Storage

All data lives in `~/.config/nw-tracker/portfolio.json` (respects `$XDG_CONFIG_HOME`), and can be overridden per invocation with `--file <PATH>` (`-f`) or the `NW_FILE` environment variable.
The file is never modified in place — writes go to a secure temporary sibling file (`0600` on Unix), are synced to disk, and atomically replace the target. Keys are sorted deterministically (`BTreeMap`) and end with a trailing newline for clean git diffs.

**Recommended backup strategy:** keep `portfolio.json` in a private git repository.

## Concepts

**Asset** — a thing you own with a stable identity: a brokerage account, a savings account, a crypto wallet, a real-estate position, etc. Each asset has an ID, a display name, a category (free-form string), a currency, and an optional `archived` flag.

**Snapshot** — a dated record of asset values. You add one whenever you want to capture a moment. Each snapshot stores:
- exchange rates for every non-USD currency in your active asset list (as `"1 USD = N foreign units"`)
- a non-negative value for each asset you want to include (assets can be omitted if unknown)

**Category** — a free-form grouping string (e.g. `etf`, `crypto`, `bank`, `cash`, `real-estate`). Stored lowercase and matched case-insensitively.

## Commands

Global flags: `-f, --file <PATH>` (`NW_FILE`), `--no-color`, `-V, --version`.

### Asset management

```sh
# Add a new asset
nw asset add -i <id> -n <name> -c <category> --currency <currency>

# Edit an existing asset (pass --force if changing currency on an asset with history)
nw asset edit -i <id> [-n <name>] [-c <category>] [--currency <currency> [--force]]

# Archive / unarchive a closed account without erasing historical net worth
nw asset archive -i <id>
nw asset unarchive -i <id>

# Remove an asset (prompts if referenced in snapshots; --purge deletes historical entries)
nw asset remove -i <id> [--purge] [-y|--yes]

# List active assets (or --all to include archived; --format table|json|csv)
nw asset list [-a|--all] [--format <table|json|csv>]
```

**Examples:**
```sh
nw asset add -i vti-brokerage -n "VTI"             -c etf  --currency USD
nw asset add -i savings-chf   -n "Savings Account" -c bank --currency CHF
nw asset add -i eur-cash      -n "EUR Cash"        -c cash --currency EUR
nw asset edit -i vti-brokerage -n "VTI (Brokerage)"
nw asset archive -i eur-cash
```

---

### Snapshot management

```sh
# Record a new snapshot (defaults to today; --carry-forward lets Enter reuse previous values)
nw snapshot add [-d <YYYY-MM-DD>] [--carry-forward]

# Edit an existing snapshot (Enter keeps value, '-' clears; previews diff before saving)
nw snapshot edit -d <YYYY-MM-DD> [-y|--yes]

# Remove an existing snapshot
nw snapshot remove -d <YYYY-MM-DD> [-y|--yes]

# List all snapshots
nw snapshot list [--format <table|json|csv>]
```

`snapshot add` and `snapshot edit` are interactive (comma/underscore inputs like `12,500.50` are accepted, and `Ctrl-D` aborts without saving):
1. For each non-USD currency in your active asset list, enter the exchange rate as `"1 USD = N units"` (e.g. for EUR: if 1 USD buys 0.92 EUR, enter `0.92`)
2. For each active asset, enter its current value in its native currency — press Enter to omit (or keep existing/previous), or `-` to clear

Snapshots are always stored in ascending date order regardless of insertion order, so backfilling old dates is safe.

---

### Display & Utilities

```sh
# Show latest snapshot (or a specific date, category filter, or json/csv format)
nw show [-d <YYYY-MM-DD>] [-c <category>] [--format <table|json|csv>]

# Show net worth history (range defaults to 1Y)
nw history [-r <1M|6M|1Y|5Y|ALL>] [-c <category>] [--format <table|json|csv>]

# Print the active portfolio file path
nw path

# Validate portfolio.json and show summary counts
nw validate

# Generate shell completions
nw completions <bash|elvish|fish|powershell|zsh>
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

TOTAL  23,369.57  (+1,200.00, +5.41% vs 2025-05-01)

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

Period change (2024-06-01 -> 2025-06-01): +9,100.00 (+21.51%)
```

History ranges: `1M` (1 month), `6M` (6 months), `1Y` (1 year, default), `5Y` (5 years), `ALL`.

---

## Data format

`portfolio.json` is human-readable and straightforward to edit by hand if needed:

```json
{
  "version": 1,
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
- `version` is schema version `1` (optional on load, added on save).
- `rates` stores non-USD currencies only, as `"1 USD = N foreign units"`. USD is always `1` by definition.
- `entries` may omit assets — partial snapshots are valid.
- `value` is exact decimal in the asset's native currency. Conversion to USD happens at display time.
- `category` is lowercase; `currency` is uppercase alphanumeric code (e.g. `USD`, `EUR`).
- Snapshots are always sorted ascending by `YYYY-MM-DD` date.

## License

BSD 3-Clause License
