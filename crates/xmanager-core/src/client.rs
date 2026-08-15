//! X (Twitter) API v2 client with OAuth 1.0a user-context auth.

use crate::config::Settings;
use crate::error::{Error, Result};
use crate::logging::{self, events, Outcome, Stream};
use crate::models::{PublicMetrics, Tweet, User};
use oauth1_request as oauth;
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::StatusCode;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const BASE_URL: &str = "https://api.x.com/2";
const BASE_URL_FALLBACK: &str = "https://api.twitter.com/2";
const TWEET_FIELDS: &str =
    "created_at,public_metrics,conversation_id,in_reply_to_user_id,referenced_tweets";

/// Live tweet lookup result (`GET /2/tweets?ids=`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TweetLookup {
    pub found: Vec<Tweet>,
    pub missing: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// Authenticated client for X API v2 (blocking; safe to call from a background thread).
pub struct XClient {
    http: Client,
    settings: Settings,
}

impl XClient {
    pub fn new(settings: Settings) -> Result<Self> {
        settings.require_oauth1()?;
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(8))
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(Self { http, settings })
    }

    /// Return the authenticated user.
    pub fn get_me(&self) -> Result<User> {
        let started = Instant::now();
        let url = format!("{BASE_URL}/users/me");
        let mut params = BTreeMap::new();
        params.insert("user.fields".into(), "id,username,name".into());
        let data = match self.request_json("GET", &url, &params) {
            Ok(data) => data,
            Err(err) => {
                logging::error(Stream::App, events::ACCOUNT_WHOAMI)
                    .outcome(Outcome::Error)
                    .field("error", err.to_string())
                    .field("duration_ms", started.elapsed().as_millis() as u64)
                    .emit();
                return Err(err);
            }
        };
        let user = data
            .get("data")
            .ok_or_else(|| Error::Parse("missing data in /users/me".into()))?;
        let user = User {
            id: json_str(user, "id")?,
            username: json_str(user, "username").unwrap_or_default(),
            name: json_str(user, "name").unwrap_or_default(),
        };
        logging::info(Stream::App, events::ACCOUNT_WHOAMI)
            .outcome(Outcome::Ok)
            .field("user_id", user.id.as_str())
            .field("username", user.username.as_str())
            .field("duration_ms", started.elapsed().as_millis() as u64)
            .emit();
        Ok(user)
    }

    /// Fetch user tweets with pagination until `limit` or end of timeline.
    pub fn fetch_user_tweets(
        &self,
        user_id: &str,
        limit: usize,
        exclude_retweets: bool,
        exclude_replies: bool,
    ) -> Result<Vec<Tweet>> {
        let started = Instant::now();
        let mut out = Vec::new();
        let mut pagination_token: Option<String> = None;
        let mut pages = 0u32;

        while out.len() < limit {
            let remaining = limit - out.len();
            let page_size = remaining.min(100).max(5);

            let mut params = BTreeMap::new();
            params.insert("max_results".into(), page_size.to_string());
            params.insert("tweet.fields".into(), TWEET_FIELDS.into());

            let mut exclude = Vec::new();
            if exclude_retweets {
                exclude.push("retweets");
            }
            if exclude_replies {
                exclude.push("replies");
            }
            if !exclude.is_empty() {
                params.insert("exclude".into(), exclude.join(","));
            }
            if let Some(token) = &pagination_token {
                params.insert("pagination_token".into(), token.clone());
            }

            let url = format!("{BASE_URL}/users/{user_id}/tweets");
            let data = match self.request_json("GET", &url, &params) {
                Ok(data) => data,
                Err(err) => {
                    logging::error(Stream::App, events::TIMELINE_FETCH)
                        .outcome(Outcome::Error)
                        .field("user_id", user_id)
                        .field("limit", limit as u64)
                        .field("pages", pages)
                        .field("count", out.len() as u64)
                        .field("error", err.to_string())
                        .field("duration_ms", started.elapsed().as_millis() as u64)
                        .emit();
                    return Err(err);
                }
            };
            pages += 1;

            let items = data
                .get("data")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if items.is_empty() {
                break;
            }

            for item in items {
                if out.len() >= limit {
                    break;
                }
                out.push(parse_tweet(&item)?);
            }

            pagination_token = data
                .get("meta")
                .and_then(|m| m.get("next_token"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());

            if pagination_token.is_none() {
                break;
            }
        }

        logging::info(Stream::App, events::TIMELINE_FETCH)
            .outcome(Outcome::Ok)
            .field("user_id", user_id)
            .field("limit", limit as u64)
            .field("count", out.len() as u64)
            .field("pages", pages)
            .field("exclude_retweets", exclude_retweets)
            .field("exclude_replies", exclude_replies)
            .field("duration_ms", started.elapsed().as_millis() as u64)
            .emit();
        Ok(out)
    }

    /// Fetch own tweets (resolves user id first).
    pub fn fetch_own_tweets(
        &self,
        limit: usize,
        exclude_retweets: bool,
        exclude_replies: bool,
    ) -> Result<(User, Vec<Tweet>)> {
        let me = self.get_me()?;
        let tweets =
            self.fetch_user_tweets(&me.id, limit, exclude_retweets, exclude_replies)?;
        Ok((me, tweets))
    }

    /// Delete a tweet by id. Returns true only when `data.deleted` is true.
    pub fn delete_tweet(&self, tweet_id: &str) -> Result<bool> {
        let url = format!("{BASE_URL}/tweets/{tweet_id}");
        let params = BTreeMap::new();
        let data = match self.request_json("DELETE", &url, &params) {
            Ok(data) => data,
            Err(err) => {
                logging::error(Stream::Audit, events::TWEET_DELETE)
                    .outcome(Outcome::Error)
                    .field("tweet_id", tweet_id)
                    .field("error", err.to_string())
                    .emit();
                return Err(err);
            }
        };
        match deleted_flag(&data) {
            Ok(deleted) => {
                logging::info(Stream::Audit, events::TWEET_DELETE)
                    .outcome(if deleted {
                        Outcome::Ok
                    } else {
                        Outcome::Error
                    })
                    .field("tweet_id", tweet_id)
                    .field("deleted", deleted)
                    .emit();
                Ok(deleted)
            }
            Err(err) => {
                logging::error(Stream::Audit, events::TWEET_DELETE)
                    .outcome(Outcome::Error)
                    .field("tweet_id", tweet_id)
                    .field("error", err.to_string())
                    .emit();
                Err(err)
            }
        }
    }

    /// Delete many tweets; returns (success_count, failures with id+error).
    ///
    /// Stops immediately on `Error::RateLimited` and marks remaining ids as skipped.
    pub fn delete_tweets(&self, ids: &[String]) -> (usize, Vec<(String, String)>) {
        let mut ok = 0usize;
        let mut fail = Vec::new();
        for (idx, id) in ids.iter().enumerate() {
            match self.delete_tweet(id) {
                Ok(true) => ok += 1,
                Ok(false) => fail.push((id.clone(), "API returned deleted=false".into())),
                Err(e) => {
                    let rate_limited = matches!(e, Error::RateLimited { .. });
                    fail.push((id.clone(), e.to_string()));
                    if rate_limited {
                        for skipped in &ids[idx + 1..] {
                            fail.push((skipped.clone(), "skipped: rate limited".into()));
                        }
                        break;
                    }
                }
            }
        }
        (ok, fail)
    }

    /// Look up tweets by id (`GET /2/tweets?ids=`), in batches of 100.
    ///
    /// Empty `ids` returns an empty result. A rate-limited batch returns
    /// `Error::RateLimited` so the caller can surface a banner.
    pub fn lookup_tweets(&self, ids: &[String]) -> Result<TweetLookup> {
        if ids.is_empty() {
            return Ok(TweetLookup::default());
        }

        let mut found = Vec::new();
        let mut missing = Vec::new();
        let mut failed = Vec::new();

        for chunk in ids.chunks(100) {
            let mut params = BTreeMap::new();
            params.insert("ids".into(), chunk.join(","));
            params.insert("tweet.fields".into(), TWEET_FIELDS.into());
            let url = format!("{BASE_URL}/tweets");
            let data = self.request_json_partial("GET", &url, &params)?;
            let classified = classify_lookup(chunk, &data);
            found.extend(classified.found);
            missing.extend(classified.missing);
            failed.extend(classified.failed);
        }

        logging::info(Stream::App, events::TWEET_LOOKUP)
            .outcome(if failed.is_empty() {
                Outcome::Ok
            } else {
                Outcome::Partial
            })
            .field("requested", ids.len() as u64)
            .field("found", found.len() as u64)
            .field("missing", missing.len() as u64)
            .field("failed", failed.len() as u64)
            .emit();
        Ok(TweetLookup {
            found,
            missing,
            failed,
        })
    }

    fn request_json(
        &self,
        method: &str,
        url: &str,
        params: &BTreeMap<String, String>,
    ) -> Result<Value> {
        self.request_json_inner(method, url, params, false)
    }

    fn request_json_partial(
        &self,
        method: &str,
        url: &str,
        params: &BTreeMap<String, String>,
    ) -> Result<Value> {
        self.request_json_inner(method, url, params, true)
    }

    fn request_json_inner(
        &self,
        method: &str,
        url: &str,
        params: &BTreeMap<String, String>,
        allow_partial_errors: bool,
    ) -> Result<Value> {
        let started = Instant::now();
        let response = match self.send(method, url, params) {
            Ok(response) => response,
            Err(err) => {
                logging::error(Stream::App, events::API_REQUEST)
                    .outcome(Outcome::Error)
                    .field("http_method", method)
                    .field("url", logging::sanitize_url(url))
                    .field("error", err.to_string())
                    .field("duration_ms", started.elapsed().as_millis() as u64)
                    .emit();
                return Err(err);
            }
        };

        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            let retry_after_secs = rate_limit_sleep_secs(&response);
            logging::warn(Stream::App, events::API_RATE_LIMITED)
                .field("http_method", method)
                .field("url", logging::sanitize_url(url))
                .field("sleep_secs", retry_after_secs)
                .field("retry", false)
                .emit();
            return Err(Error::RateLimited { retry_after_secs });
        }

        let status = response.status();
        let body = response
            .text()
            .map_err(|e| Error::Network(e.to_string()))?;
        let duration_ms = started.elapsed().as_millis() as u64;
        let url_path = logging::sanitize_url(url);

        if status == StatusCode::NO_CONTENT || body.trim().is_empty() {
            if status.is_success() {
                logging::debug(Stream::App, events::API_REQUEST)
                    .outcome(Outcome::Ok)
                    .field("http_method", method)
                    .field("url", url_path)
                    .field("http_status", status.as_u16())
                    .field("duration_ms", duration_ms)
                    .emit();
                return Ok(Value::Null);
            }
            logging::error(Stream::App, events::API_REQUEST)
                .outcome(Outcome::Error)
                .field("http_method", method)
                .field("url", url_path)
                .field("http_status", status.as_u16())
                .field("error", "empty body")
                .field("duration_ms", duration_ms)
                .emit();
            return Err(Error::api(status.as_u16(), "empty body"));
        }

        let data: Value = match serde_json::from_str(&body) {
            Ok(data) => data,
            Err(e) => {
                let msg = format!(
                    "invalid JSON (HTTP {}): {e}; body={}",
                    status.as_u16(),
                    truncate(&body, 200)
                );
                logging::error(Stream::App, events::API_REQUEST)
                    .outcome(Outcome::Error)
                    .field("http_method", method)
                    .field("url", url_path)
                    .field("http_status", status.as_u16())
                    .field("error", msg.as_str())
                    .field("duration_ms", duration_ms)
                    .emit();
                return Err(Error::Parse(msg));
            }
        };

        if !status.is_success() {
            let msg = format_api_error(&data, status.as_u16());
            logging::error(Stream::App, events::API_REQUEST)
                .outcome(Outcome::Error)
                .field("http_method", method)
                .field("url", url_path.as_str())
                .field("http_status", status.as_u16())
                .field("error", msg.as_str())
                .field("duration_ms", duration_ms)
                .emit();
            return Err(Error::api(status.as_u16(), msg));
        }

        if !allow_partial_errors && has_errors_without_usable_data(&data) {
            let msg = format_api_error(&data, status.as_u16());
            logging::error(Stream::App, events::API_REQUEST)
                .outcome(Outcome::Error)
                .field("http_method", method)
                .field("url", url_path.as_str())
                .field("http_status", status.as_u16())
                .field("error", msg.as_str())
                .field("duration_ms", duration_ms)
                .emit();
            return Err(Error::api(status.as_u16(), msg));
        }

        logging::debug(Stream::App, events::API_REQUEST)
            .outcome(Outcome::Ok)
            .field("http_method", method)
            .field("url", url_path)
            .field("http_status", status.as_u16())
            .field("duration_ms", duration_ms)
            .emit();
        Ok(data)
    }

    fn send(
        &self,
        method: &str,
        url: &str,
        params: &BTreeMap<String, String>,
    ) -> Result<Response> {
        let mut last_network = None;
        for candidate in candidate_urls(url) {
            match self.send_once(method, &candidate, params) {
                Ok(response) => {
                    logging::debug(Stream::App, events::API_REQUEST)
                        .outcome(Outcome::Ok)
                        .field("http_method", method)
                        .field("url", logging::sanitize_url(&candidate))
                        .field("http_status", response.status().as_u16())
                        .field("host_fallback", candidate != url)
                        .emit();
                    return Ok(response);
                }
                Err(err) if matches!(err, Error::Network(_)) => {
                    logging::warn(Stream::App, events::API_REQUEST)
                        .outcome(Outcome::Error)
                        .field("http_method", method)
                        .field("url", logging::sanitize_url(&candidate))
                        .field("error", err.to_string())
                        .field("will_fallback", true)
                        .emit();
                    last_network = Some(err);
                }
                Err(err) => {
                    logging::error(Stream::App, events::API_REQUEST)
                        .outcome(Outcome::Error)
                        .field("http_method", method)
                        .field("url", logging::sanitize_url(&candidate))
                        .field("error", err.to_string())
                        .emit();
                    return Err(err);
                }
            }
        }
        Err(last_network.unwrap_or_else(|| Error::Network("no API host remaining".into())))
    }

    fn send_once(
        &self,
        method: &str,
        url: &str,
        params: &BTreeMap<String, String>,
    ) -> Result<Response> {
        let auth = sign_oauth1(
            method,
            url,
            params,
            &self.settings.api_key,
            &self.settings.api_secret,
            &self.settings.access_token,
            &self.settings.access_token_secret,
        )?;

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth).map_err(|e| Error::Other(e.to_string()))?,
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("XManager/0.1.0"));

        let mut req = match method {
            "GET" => self.http.get(url),
            "DELETE" => self.http.delete(url),
            "POST" => self.http.post(url),
            other => return Err(Error::Other(format!("unsupported method {other}"))),
        };
        req = req.headers(headers);
        if !params.is_empty() {
            req = req.query(&params.iter().collect::<Vec<_>>());
        }

        req.send().map_err(|e| Error::Network(e.to_string()))
    }
}

fn candidate_urls(url: &str) -> Vec<String> {
    let alt = if url.starts_with(BASE_URL) {
        url.replacen(BASE_URL, BASE_URL_FALLBACK, 1)
    } else if url.starts_with(BASE_URL_FALLBACK) {
        url.replacen(BASE_URL_FALLBACK, BASE_URL, 1)
    } else {
        return vec![url.to_string()];
    };
    vec![url.to_string(), alt]
}

fn sign_oauth1(
    method: &str,
    url: &str,
    params: &BTreeMap<String, String>,
    consumer_key: &str,
    consumer_secret: &str,
    token: &str,
    token_secret: &str,
) -> Result<String> {
    // `uri` must not contain a query part — params go into the Request object.
    let oauth_token = oauth::Token::from_parts(
        consumer_key,
        consumer_secret,
        token,
        token_secret,
    );

    // ParameterList sorts keys for a correct signature base string.
    let pairs: Vec<(&str, &str)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let request = oauth::ParameterList::new(pairs);

    let authorization = oauth::authorize(method, url, &request, &oauth_token, oauth::HMAC_SHA1);
    Ok(authorization)
}

fn rate_limit_sleep_secs(response: &Response) -> u64 {
    if let Some(reset) = response
        .headers()
        .get("x-rate-limit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        return reset.saturating_sub(now).saturating_add(1).min(900);
    }
    if let Some(retry) = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        return retry.min(900);
    }
    60
}

fn format_api_error(data: &Value, status: u16) -> String {
    let title = data
        .get("title")
        .or_else(|| data.get("error"))
        .and_then(|v| v.as_str());
    let detail = data
        .get("detail")
        .or_else(|| data.get("message"))
        .and_then(|v| v.as_str());
    let first_err = data
        .get("errors")
        .and_then(|e| e.as_array())
        .and_then(|a| a.first())
        .and_then(|e| {
            e.get("message")
                .or_else(|| e.get("detail"))
                .and_then(|v| v.as_str())
        });

    let mut parts = vec![format!("HTTP {status}")];
    if status == 429 {
        parts.push("触发速率限制，请稍后重试".into());
    }
    if let Some(t) = title {
        parts.push(t.to_string());
    }
    if let Some(d) = detail {
        parts.push(d.to_string());
    }
    if let Some(e) = first_err {
        parts.push(e.to_string());
    }
    parts.join(" — ")
}

fn parse_tweet(item: &Value) -> Result<Tweet> {
    let metrics_raw = item.get("public_metrics").cloned().unwrap_or(Value::Null);
    let metrics: PublicMetrics = serde_json::from_value(metrics_raw).unwrap_or_default();
    let (is_retweet, is_quote) = parse_referenced_flags(item);
    Ok(Tweet {
        id: json_str(item, "id")?,
        text: item
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        created_at: item
            .get("created_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        public_metrics: metrics,
        conversation_id: item
            .get("conversation_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        in_reply_to_user_id: item
            .get("in_reply_to_user_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        is_retweet,
        is_quote,
    })
}

/// Read `referenced_tweets` for retweeted / quoted flags.
fn parse_referenced_flags(item: &Value) -> (bool, bool) {
    let mut is_retweet = false;
    let mut is_quote = false;
    if let Some(arr) = item.get("referenced_tweets").and_then(|v| v.as_array()) {
        for ref_t in arr {
            match ref_t.get("type").and_then(|v| v.as_str()) {
                Some("retweeted") => is_retweet = true,
                Some("quoted") => is_quote = true,
                // "replied_to" is already covered by in_reply_to_user_id
                _ => {}
            }
        }
    }
    (is_retweet, is_quote)
}

/// Require `data.deleted` to be a boolean. Missing / null / empty is an error.
fn deleted_flag(data: &Value) -> Result<bool> {
    data.get("data")
        .and_then(|d| d.get("deleted"))
        .and_then(|v| v.as_bool())
        .ok_or_else(|| {
            Error::Parse("delete response missing data.deleted (empty or malformed body)".into())
        })
}

fn has_usable_data(data: &Value) -> bool {
    match data.get("data") {
        None | Some(Value::Null) => false,
        Some(Value::Array(items)) => !items.is_empty(),
        Some(_) => true,
    }
}

fn has_errors_without_usable_data(data: &Value) -> bool {
    let has_errors = data
        .get("errors")
        .and_then(|e| e.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    has_errors && !has_usable_data(data)
}

fn classify_lookup(requested: &[String], body: &Value) -> TweetLookup {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    let mut failed = Vec::new();
    let mut accounted = HashSet::new();

    if let Some(items) = body.get("data").and_then(|v| v.as_array()) {
        for item in items {
            match parse_tweet(item) {
                Ok(tweet) => {
                    accounted.insert(tweet.id.clone());
                    found.push(tweet);
                }
                Err(e) => {
                    if let Ok(id) = json_str(item, "id") {
                        accounted.insert(id.clone());
                        failed.push((id, e.to_string()));
                    }
                }
            }
        }
    }

    if let Some(errors) = body.get("errors").and_then(|v| v.as_array()) {
        for err in errors {
            let Some(id) = lookup_error_id(err) else {
                continue;
            };
            if !accounted.insert(id.clone()) {
                continue;
            }
            if is_not_found_error(err) {
                missing.push(id);
            } else {
                failed.push((id, lookup_error_message(err)));
            }
        }
    }

    for id in requested {
        if !accounted.contains(id) {
            failed.push((id.clone(), "not returned by tweet lookup".into()));
        }
    }

    TweetLookup {
        found,
        missing,
        failed,
    }
}

fn lookup_error_id(err: &Value) -> Option<String> {
    err.get("resource_id")
        .or_else(|| err.get("value"))
        .and_then(|v| {
            v.as_str()
                .map(str::to_string)
                .or_else(|| v.as_u64().map(|n| n.to_string()))
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        })
}

fn is_not_found_error(err: &Value) -> bool {
    let ty = err.get("type").and_then(Value::as_str).unwrap_or("");
    let title = err.get("title").and_then(Value::as_str).unwrap_or("");
    let detail = err
        .get("detail")
        .or_else(|| err.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let ty_l = ty.to_ascii_lowercase();
    let title_l = title.to_ascii_lowercase();
    let detail_l = detail.to_ascii_lowercase();
    ty_l.contains("resource-not-found")
        || ty_l.contains("not-found")
        || title_l.contains("not found")
        || detail_l.contains("could not find")
        || err.get("status").and_then(Value::as_u64) == Some(404)
}

fn lookup_error_message(err: &Value) -> String {
    err.get("detail")
        .or_else(|| err.get("message"))
        .or_else(|| err.get("title"))
        .and_then(Value::as_str)
        .unwrap_or("tweet lookup error")
        .to_string()
}

fn json_str(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.as_u64().map(|n| n.to_string()))
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        })
        .ok_or_else(|| Error::Parse(format!("missing field `{key}`")))
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_tweet_reads_metrics_and_referenced_flags() {
        let item = json!({
            "id": "123",
            "text": "hello world",
            "created_at": "2024-01-02T03:04:05.000Z",
            "public_metrics": {
                "retweet_count": 1,
                "reply_count": 2,
                "like_count": 3,
                "quote_count": 4,
                "bookmark_count": 5,
                "impression_count": 50
            },
            "conversation_id": "123",
            "in_reply_to_user_id": null,
            "referenced_tweets": [
                { "type": "quoted", "id": "999" }
            ]
        });
        let tweet = parse_tweet(&item).expect("parse tweet");
        assert_eq!(tweet.id, "123");
        assert_eq!(tweet.text, "hello world");
        assert_eq!(tweet.views(), 50);
        assert_eq!(tweet.public_metrics.like_count, 3);
        assert!(tweet.is_quote);
        assert!(!tweet.is_retweet);
        assert_eq!(tweet.kind(), crate::models::PostKind::Quote);
    }

    #[test]
    fn parse_referenced_flags_detects_retweet() {
        let item = json!({
            "referenced_tweets": [
                { "type": "retweeted", "id": "1" },
                { "type": "replied_to", "id": "2" }
            ]
        });
        let (is_rt, is_quote) = parse_referenced_flags(&item);
        assert!(is_rt);
        assert!(!is_quote);
    }

    #[test]
    fn json_str_accepts_numeric_ids() {
        let value = json!({ "id": 42 });
        assert_eq!(json_str(&value, "id").unwrap(), "42");
    }

    #[test]
    fn format_api_error_mentions_rate_limit_on_429() {
        let msg = format_api_error(&json!({ "title": "Too Many Requests" }), 429);
        assert!(msg.contains("HTTP 429"));
        assert!(msg.contains("速率限制"));
        assert!(msg.contains("Too Many Requests"));
    }

    #[test]
    fn candidate_urls_try_x_then_twitter() {
        let urls = candidate_urls("https://api.x.com/2/users/me");
        assert_eq!(
            urls,
            vec![
                "https://api.x.com/2/users/me".to_string(),
                "https://api.twitter.com/2/users/me".to_string()
            ]
        );
    }

    #[test]
    fn lookup_tweets_empty_ids_is_empty() {
        let client = XClient::new(Settings {
            api_key: "k".into(),
            api_secret: "s".into(),
            access_token: "t".into(),
            access_token_secret: "ts".into(),
            bearer_token: String::new(),
        })
        .expect("dummy client");
        let lookup = client.lookup_tweets(&[]).expect("empty lookup");
        assert!(lookup.found.is_empty());
        assert!(lookup.missing.is_empty());
        assert!(lookup.failed.is_empty());
    }

    #[test]
    fn rate_limited_display_contains_retry_seconds() {
        let err = Error::RateLimited {
            retry_after_secs: 42,
        };
        let msg = err.to_string();
        assert!(msg.contains("42"), "{msg}");
        assert!(
            msg.to_ascii_lowercase().contains("rate limited") || msg.contains("限流"),
            "{msg}"
        );
    }

    #[test]
    fn deleted_flag_missing_is_not_success() {
        assert!(deleted_flag(&Value::Null).is_err());
        assert!(deleted_flag(&json!({})).is_err());
        assert!(deleted_flag(&json!({ "data": {} })).is_err());
        assert!(deleted_flag(&json!({ "data": { "deleted": null } })).is_err());
        assert_ne!(deleted_flag(&Value::Null).ok(), Some(true));
        assert_eq!(
            deleted_flag(&json!({ "data": { "deleted": true } })).unwrap(),
            true
        );
        assert_eq!(
            deleted_flag(&json!({ "data": { "deleted": false } })).unwrap(),
            false
        );
    }

    #[test]
    fn classify_lookup_splits_found_missing_failed() {
        let body = json!({
            "data": [
                {
                    "id": "1",
                    "text": "hello",
                    "public_metrics": { "like_count": 1 }
                }
            ],
            "errors": [
                {
                    "resource_id": "2",
                    "value": "2",
                    "title": "Not Found Error",
                    "detail": "Could not find tweet with id: [2].",
                    "type": "https://api.twitter.com/2/problems/resource-not-found"
                },
                {
                    "resource_id": "3",
                    "title": "Authorization Error",
                    "detail": "not authorized to view this tweet",
                    "type": "https://api.twitter.com/2/problems/not-authorized-for-resource"
                }
            ]
        });
        let ids = vec![
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ];
        let lookup = classify_lookup(&ids, &body);
        assert_eq!(lookup.found.len(), 1);
        assert_eq!(lookup.found[0].id, "1");
        assert_eq!(lookup.missing, vec!["2".to_string()]);
        assert_eq!(lookup.failed.len(), 2);
        assert_eq!(lookup.failed[0].0, "3");
        assert!(lookup.failed[0].1.contains("not authorized"));
        assert_eq!(lookup.failed[1].0, "4");
    }

    #[test]
    fn errors_without_usable_data_is_detected() {
        assert!(has_errors_without_usable_data(&json!({
            "errors": [{ "detail": "boom" }]
        })));
        assert!(has_errors_without_usable_data(&json!({
            "data": [],
            "errors": [{ "detail": "boom" }]
        })));
        assert!(!has_errors_without_usable_data(&json!({
            "data": [{ "id": "1" }],
            "errors": [{ "detail": "partial" }]
        })));
        assert!(!has_errors_without_usable_data(&json!({
            "data": []
        })));
    }
}

