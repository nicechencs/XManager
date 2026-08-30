//! XManager core: X API client, tweet models, filtering, export, and logging.

pub mod client;
pub mod config;
pub mod error;
pub mod export;
pub mod filter;
pub mod logging;
pub mod models;
pub mod paths;

pub use client::{TweetLookup, XClient};
pub use config::{looks_like_user_access_token, CredentialLayout, CredentialPresence, Settings};
pub use error::{Error, Result};
pub use export::{export_auto, export_csv, export_json, load_tweets_file, parse_tweets_json};
pub use filter::{
    filter_tweets, parse_created_at, summarize, view_bucket_bounds, view_histogram, FilterOptions,
    KindFilter, SortField, SortOrder, Summary, TimeRange, VIEW_BUCKETS,
};
pub use logging::{Level as LogLevel, LogConfig, Outcome as LogOutcome, Stream as LogStream};
pub use models::{
    coalesce_impression_count, NonPublicMetrics, PostKind, PublicMetrics, Tweet, User,
};
pub use paths::{env_placement_hint_zh, PathResolver};
