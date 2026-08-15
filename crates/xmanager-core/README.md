# xmanager-core

XManager domain library: X API v2 client (OAuth 1.0a), tweet models, filtering, CSV/JSON export.

## Modules

| Module | Role |
|--------|------|
| `config` | Load `.env` / env vars (`X_API_KEY` …) |
| `client` | Blocking `XClient`: `get_me`, `fetch_own_tweets`, `delete_tweet` |
| `models` | `Tweet`, `User`, `PublicMetrics` |
| `filter` | `filter_tweets`, `summarize`, histogram buckets |
| `export` | `export_csv` / `export_json` / `export_auto` / `parse_tweets_json` |

命令行入口：`cargo run -p xmanager-cli -- --help`。

## Usage

```rust
use xmanager_core::{Settings, XClient, FilterOptions, filter_tweets, export_csv};

let settings = Settings::load()?;
let client = XClient::new(settings)?;
let (me, tweets) = client.fetch_own_tweets(100, true, true)?;
let low = filter_tweets(&tweets, &FilterOptions::low_views(50));
export_csv(&low, "exports/low.csv")?;
```
