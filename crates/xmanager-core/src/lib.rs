//! XManager core: X API client, tweet models, filtering, and export.

pub mod client;
pub mod config;
pub mod error;
pub mod export;
pub mod filter;
pub mod models;

pub use client::XClient;
pub use config::Settings;
pub use error::{Error, Result};
pub use export::{export_auto, export_csv, export_json};
pub use filter::{
    filter_tweets, parse_created_at, summarize, view_bucket_bounds, view_histogram, FilterOptions,
    KindFilter, SortField, SortOrder, Summary, TimeRange, VIEW_BUCKETS,
};
pub use models::{PostKind, PublicMetrics, Tweet, User};
