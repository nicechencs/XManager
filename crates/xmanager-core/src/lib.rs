//! XManager core: X API client, tweet models, filtering, export, and logging.

pub mod client;
pub mod config;
pub mod error;
pub mod export;
pub mod filter;
pub mod logging;
pub mod models;

pub use client::{TweetLookup, XClient};
pub use config::Settings;
pub use error::{Error, Result};
pub use export::{export_auto, export_csv, export_json, load_tweets_file, parse_tweets_json};
pub use filter::{
    filter_tweets, parse_created_at, summarize, view_bucket_bounds, view_histogram, FilterOptions,
    KindFilter, SortField, SortOrder, Summary, TimeRange, VIEW_BUCKETS,
};
pub use logging::{LogConfig, Level as LogLevel, Outcome as LogOutcome, Stream as LogStream};
pub use models::{PostKind, PublicMetrics, Tweet, User};
