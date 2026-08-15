//! XManager CLI — JSON-first commands for automation and tests.
//!
//! Live X API calls (`whoami`, `fetch`, `delete --yes`) need OAuth 1.0a env.
//! `filter` / `summarize` / `export` / `creds` / `delete` dry-run stay offline.

use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use xmanager_core::{
    export_auto, filter_tweets, load_tweets_file, parse_tweets_json, summarize, FilterOptions,
    KindFilter, Settings, SortField, SortOrder, TimeRange, Tweet, XClient,
};

const EXIT_OK: u8 = 0;
const EXIT_ERROR: u8 = 1;
const EXIT_CREDS: u8 = 2;
const EXIT_API: u8 = 3;

#[derive(Parser, Debug)]
#[command(
    name = "xmanager-cli",
    version,
    about = "XManager command line (JSON stdout). Desktop UI is a separate binary."
)]
struct Cli {
    /// Load credentials from this .env instead of walking the working directory.
    #[arg(long, global = true, value_name = "PATH")]
    env: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Check OAuth 1.0a env presence (no network).
    Creds,
    /// GET /2/users/me
    Whoami,
    /// Fetch the authenticated user's tweets.
    Fetch {
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        exclude_retweets: bool,
        #[arg(long)]
        exclude_replies: bool,
        /// Optional tweet-array JSON file.
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
    /// Filter tweets from a JSON file or stdin (`-`).
    Filter {
        #[arg(long, short, default_value = "-")]
        input: String,
        #[arg(long)]
        max_views: Option<u64>,
        #[arg(long)]
        min_views: Option<u64>,
        #[arg(long)]
        max_engagement: Option<u64>,
        #[arg(long)]
        older_than_days: Option<u64>,
        #[arg(long, value_enum)]
        time_range: Option<TimeRangeArg>,
        /// Comma list: original,reply,retweet,quote
        #[arg(long, value_delimiter = ',')]
        kinds: Option<Vec<KindArg>>,
        #[arg(long, value_enum, default_value_t = SortArg::Views)]
        sort: SortArg,
        #[arg(long, value_enum, default_value_t = OrderArg::Asc)]
        order: OrderArg,
        #[arg(long)]
        top: Option<usize>,
        /// Shortcut matching the UI low-view preset (also drops replies/retweets).
        #[arg(long)]
        low_views: Option<u64>,
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
    /// Summary statistics from a tweet JSON file or stdin.
    Summarize {
        #[arg(long, short, default_value = "-")]
        input: String,
    },
    /// Write tweets to CSV or JSON (extension selects format).
    Export {
        #[arg(long, short, default_value = "-")]
        input: String,
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Delete tweets. Dry-run unless `--yes` is passed.
    Delete {
        #[arg(long, value_delimiter = ',')]
        ids: Vec<String>,
        /// Tweet JSON whose `id` fields are deleted.
        #[arg(long, short)]
        input: Option<String>,
        /// Actually call DELETE. Without this flag nothing is deleted.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum KindArg {
    Original,
    Reply,
    Retweet,
    Quote,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum, Default)]
enum SortArg {
    #[default]
    Views,
    Engagement,
    Date,
    LikeRate,
    BookmarkRate,
    EngagementRate,
    RetweetRate,
    ReplyRate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum, Default)]
enum OrderArg {
    #[default]
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum TimeRangeArg {
    All,
    #[value(name = "24h")]
    Hours24,
    #[value(name = "7d")]
    Days7,
    #[value(name = "30d")]
    Days30,
    #[value(name = "90d")]
    Days90,
    #[value(name = "180d")]
    Days180,
    #[value(name = "360d")]
    Days360,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::from(EXIT_OK),
        Err(code) => ExitCode::from(code),
    }
}

fn run(cli: Cli) -> Result<(), u8> {
    match cli.command {
        Commands::Creds => cmd_creds(cli.env.as_deref()),
        Commands::Whoami => cmd_whoami(cli.env.as_deref()),
        Commands::Fetch {
            limit,
            exclude_retweets,
            exclude_replies,
            out,
        } => cmd_fetch(
            cli.env.as_deref(),
            limit,
            exclude_retweets,
            exclude_replies,
            out.as_deref(),
        ),
        Commands::Filter {
            input,
            max_views,
            min_views,
            max_engagement,
            older_than_days,
            time_range,
            kinds,
            sort,
            order,
            top,
            low_views,
            out,
        } => {
            let tweets = read_tweets(&input)?;
            let opts = build_filter(
                max_views,
                min_views,
                max_engagement,
                older_than_days,
                time_range,
                kinds.as_deref(),
                sort,
                order,
                top,
                low_views,
            );
            let filtered = filter_tweets(&tweets, &opts);
            let summary = summarize(&filtered);
            if let Some(path) = out.as_ref() {
                write_out(&filtered, path)?;
            }
            emit_ok(json!({
                "command": "filter",
                "count": filtered.len(),
                "input_count": tweets.len(),
                "summary": summary,
                "tweets": filtered,
            }))
        }
        Commands::Summarize { input } => {
            let tweets = read_tweets(&input)?;
            emit_ok(json!({
                "command": "summarize",
                "summary": summarize(&tweets),
            }))
        }
        Commands::Export { input, out } => {
            let tweets = read_tweets(&input)?;
            let path = export_auto(&tweets, &out)
                .map_err(|e| fail(EXIT_ERROR, "io", json!({ "error": e.to_string() })))?;
            emit_ok(json!({
                "command": "export",
                "count": tweets.len(),
                "path": path,
            }))
        }
        Commands::Delete { ids, input, yes } => cmd_delete(cli.env.as_deref(), ids, input, yes),
    }
}

fn cmd_creds(env_path: Option<&Path>) -> Result<(), u8> {
    let settings = load_settings(env_path)?;
    let missing = settings.missing_oauth1();
    let payload = json!({
        "command": "creds",
        "oauth1": missing.is_empty(),
        "missing": missing,
        "has_bearer": !settings.bearer_token.is_empty(),
    });
    if missing.is_empty() {
        emit_ok(payload)
    } else {
        emit_err(EXIT_CREDS, "missing_credentials", payload)
    }
}

fn cmd_whoami(env_path: Option<&Path>) -> Result<(), u8> {
    let client = api_client(env_path)?;
    match client.get_me() {
        Ok(user) => emit_ok(json!({ "command": "whoami", "user": user })),
        Err(e) => api_fail(e),
    }
}

fn cmd_fetch(
    env_path: Option<&Path>,
    limit: usize,
    exclude_retweets: bool,
    exclude_replies: bool,
    out: Option<&Path>,
) -> Result<(), u8> {
    let client = api_client(env_path)?;
    match client.fetch_own_tweets(limit, exclude_retweets, exclude_replies) {
        Ok((user, tweets)) => {
            if let Some(path) = out {
                write_out(&tweets, path)?;
            }
            emit_ok(json!({
                "command": "fetch",
                "user": user,
                "count": tweets.len(),
                "tweets": tweets,
            }))
        }
        Err(e) => api_fail(e),
    }
}

fn cmd_delete(
    env_path: Option<&Path>,
    mut ids: Vec<String>,
    input: Option<String>,
    yes: bool,
) -> Result<(), u8> {
    if let Some(path) = input.as_deref() {
        let tweets = read_tweets(path)?;
        ids.extend(tweets.into_iter().map(|t| t.id));
    }
    ids.retain(|id| !id.is_empty());
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return emit_err(
            EXIT_ERROR,
            "empty_ids",
            json!({ "error": "no tweet ids given (use --ids or --input)" }),
        );
    }

    if !yes {
        return emit_ok(json!({
            "command": "delete",
            "dry_run": true,
            "count": ids.len(),
            "ids": ids,
        }));
    }

    let client = api_client(env_path)?;
    let (ok, failed) = client.delete_tweets(&ids);
    let failed_json: Vec<Value> = failed
        .into_iter()
        .map(|(id, error)| json!({ "id": id, "error": error }))
        .collect();
    let payload = json!({
        "command": "delete",
        "dry_run": false,
        "deleted": ok,
        "failed": failed_json,
    });
    if failed_json.is_empty() {
        emit_ok(payload)
    } else {
        emit_err(EXIT_API, "delete_partial", payload)
    }
}

fn api_client(env_path: Option<&Path>) -> Result<XClient, u8> {
    let settings = load_settings(env_path)?;
    if let Err(e) = settings.require_oauth1() {
        return Err(fail(
            EXIT_CREDS,
            "missing_credentials",
            json!({ "error": e.to_string(), "missing": settings.missing_oauth1() }),
        ));
    }
    XClient::new(settings).map_err(|e| fail(EXIT_ERROR, "client", json!({ "error": e.to_string() })))
}

fn load_settings(env_path: Option<&Path>) -> Result<Settings, u8> {
    match env_path {
        Some(path) => Settings::load_from(path),
        None => Settings::load(),
    }
    .map_err(|e| fail(EXIT_ERROR, "config", json!({ "error": e.to_string() })))
}

fn read_tweets(input: &str) -> Result<Vec<Tweet>, u8> {
    if input == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| fail(EXIT_ERROR, "io", json!({ "error": e.to_string() })))?;
        parse_tweets_json(&buf)
    } else {
        load_tweets_file(input)
    }
    .map_err(|e| fail(EXIT_ERROR, "parse", json!({ "error": e.to_string() })))
}

fn write_out(tweets: &[Tweet], path: &Path) -> Result<(), u8> {
    export_auto(tweets, path)
        .map(|_| ())
        .map_err(|e| fail(EXIT_ERROR, "io", json!({ "error": e.to_string() })))
}

fn build_filter(
    max_views: Option<u64>,
    min_views: Option<u64>,
    max_engagement: Option<u64>,
    older_than_days: Option<u64>,
    time_range: Option<TimeRangeArg>,
    kinds: Option<&[KindArg]>,
    sort: SortArg,
    order: OrderArg,
    top: Option<usize>,
    low_views: Option<u64>,
) -> FilterOptions {
    let mut opts = if let Some(threshold) = low_views {
        FilterOptions::low_views(threshold)
    } else {
        FilterOptions {
            kinds: KindFilter::all(),
            include_replies: true,
            ..FilterOptions::default()
        }
    };
    if let Some(v) = max_views {
        opts.max_views = Some(v);
    }
    if let Some(v) = min_views {
        opts.min_views = Some(v);
    }
    if let Some(v) = max_engagement {
        opts.max_engagement = Some(v);
    }
    if let Some(v) = older_than_days {
        opts.older_than_days = Some(v);
    }
    if let Some(range) = time_range {
        opts.time_range = match range {
            TimeRangeArg::All => TimeRange::All,
            TimeRangeArg::Hours24 => TimeRange::Hours24,
            TimeRangeArg::Days7 => TimeRange::Days7,
            TimeRangeArg::Days30 => TimeRange::Days30,
            TimeRangeArg::Days90 => TimeRange::Days90,
            TimeRangeArg::Days180 => TimeRange::Days180,
            TimeRangeArg::Days360 => TimeRange::Days360,
        };
    }
    if let Some(kinds) = kinds {
        opts.kinds = KindFilter {
            original: kinds.contains(&KindArg::Original),
            reply: kinds.contains(&KindArg::Reply),
            retweet: kinds.contains(&KindArg::Retweet),
            quote: kinds.contains(&KindArg::Quote),
        };
        opts.include_replies = opts.kinds.reply;
    }
    opts.sort = match sort {
        SortArg::Views => SortField::Views,
        SortArg::Engagement => SortField::Engagement,
        SortArg::Date => SortField::Date,
        SortArg::LikeRate => SortField::LikeRate,
        SortArg::BookmarkRate => SortField::BookmarkRate,
        SortArg::EngagementRate => SortField::EngagementRate,
        SortArg::RetweetRate => SortField::RetweetRate,
        SortArg::ReplyRate => SortField::ReplyRate,
    };
    opts.order = match order {
        OrderArg::Asc => SortOrder::Asc,
        OrderArg::Desc => SortOrder::Desc,
    };
    opts.top_n = top;
    opts
}

fn emit_ok(mut payload: Value) -> Result<(), u8> {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("ok".into(), json!(true));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{\"ok\":true}".into())
    );
    Ok(())
}

fn emit_err(exit: u8, code: &str, payload: Value) -> Result<(), u8> {
    Err(fail(exit, code, payload))
}

fn fail(exit: u8, code: &str, mut payload: Value) -> u8 {
    if let Some(obj) = payload.as_object_mut() {
        obj.entry("ok").or_insert(json!(false));
        obj.insert("code".into(), json!(code));
        if !obj.contains_key("error") {
            obj.insert("error".into(), json!(code));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| format!(
            "{{\"ok\":false,\"code\":\"{code}\"}}"
        ))
    );
    exit
}

fn api_fail(e: xmanager_core::Error) -> Result<(), u8> {
    use xmanager_core::Error;
    match e {
        Error::MissingCredentials(_) => Err(fail(
            EXIT_CREDS,
            "missing_credentials",
            json!({ "error": e.to_string() }),
        )),
        Error::RateLimited { retry_after_secs } => Err(fail(
            EXIT_API,
            "rate_limited",
            json!({
                "error": e.to_string(),
                "retry_after_secs": retry_after_secs
            }),
        )),
        Error::Api { .. } | Error::Network(_) => {
            Err(fail(EXIT_API, "api", json!({ "error": e.to_string() })))
        }
        _ => Err(fail(EXIT_ERROR, "error", json!({ "error": e.to_string() }))),
    }
}
