//! X (Twitter) API v2 client with OAuth 1.0a user-context auth.

use crate::config::Settings;
use crate::error::{Error, Result};
use crate::models::{PublicMetrics, Tweet, User};
use oauth1_request as oauth;
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::StatusCode;
use serde_json::Value;
use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BASE_URL: &str = "https://api.x.com/2";
const TWEET_FIELDS: &str =
    "created_at,public_metrics,conversation_id,in_reply_to_user_id,referenced_tweets";

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
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(Self { http, settings })
    }

    /// Return the authenticated user.
    pub fn get_me(&self) -> Result<User> {
        let url = format!("{BASE_URL}/users/me");
        let mut params = BTreeMap::new();
        params.insert("user.fields".into(), "id,username,name".into());
        let data = self.request_json("GET", &url, &params, true)?;
        let user = data
            .get("data")
            .ok_or_else(|| Error::Parse("missing data in /users/me".into()))?;
        Ok(User {
            id: json_str(user, "id")?,
            username: json_str(user, "username").unwrap_or_default(),
            name: json_str(user, "name").unwrap_or_default(),
        })
    }

    /// Fetch user tweets with pagination until `limit` or end of timeline.
    pub fn fetch_user_tweets(
        &self,
        user_id: &str,
        limit: usize,
        exclude_retweets: bool,
        exclude_replies: bool,
    ) -> Result<Vec<Tweet>> {
        let mut out = Vec::new();
        let mut pagination_token: Option<String> = None;

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
            let data = self.request_json("GET", &url, &params, true)?;

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

    /// Delete a tweet by id. Returns true on success.
    pub fn delete_tweet(&self, tweet_id: &str) -> Result<bool> {
        let url = format!("{BASE_URL}/tweets/{tweet_id}");
        let params = BTreeMap::new();
        let data = self.request_json("DELETE", &url, &params, true)?;
        // { "data": { "deleted": true } }
        if let Some(deleted) = data
            .get("data")
            .and_then(|d| d.get("deleted"))
            .and_then(|v| v.as_bool())
        {
            return Ok(deleted);
        }
        // empty / missing → treat as success if we got here without error
        Ok(true)
    }

    /// Delete many tweets; returns (success_count, failures with id+error).
    pub fn delete_tweets(&self, ids: &[String]) -> (usize, Vec<(String, String)>) {
        let mut ok = 0usize;
        let mut fail = Vec::new();
        for id in ids {
            match self.delete_tweet(id) {
                Ok(true) => ok += 1,
                Ok(false) => fail.push((id.clone(), "API returned deleted=false".into())),
                Err(e) => fail.push((id.clone(), e.to_string())),
            }
        }
        (ok, fail)
    }

    fn request_json(
        &self,
        method: &str,
        url: &str,
        params: &BTreeMap<String, String>,
        allow_retry: bool,
    ) -> Result<Value> {
        let response = self.send(method, url, params)?;

        if response.status() == StatusCode::TOO_MANY_REQUESTS && allow_retry {
            let sleep_secs = rate_limit_sleep_secs(&response);
            thread::sleep(Duration::from_secs(sleep_secs));
            return self.request_json(method, url, params, false);
        }

        let status = response.status();
        let body = response
            .text()
            .map_err(|e| Error::Network(e.to_string()))?;

        if status == StatusCode::NO_CONTENT || body.trim().is_empty() {
            if status.is_success() {
                return Ok(Value::Null);
            }
            return Err(Error::api(status.as_u16(), "empty body"));
        }

        let data: Value = serde_json::from_str(&body).map_err(|e| {
            Error::Parse(format!(
                "invalid JSON (HTTP {}): {e}; body={}",
                status.as_u16(),
                truncate(&body, 200)
            ))
        })?;

        if !status.is_success() {
            let msg = format_api_error(&data, status.as_u16());
            return Err(Error::api(status.as_u16(), msg));
        }

        Ok(data)
    }

    fn send(
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
            // query string for GET/DELETE
            req = req.query(&params.iter().collect::<Vec<_>>());
        }

        req.send().map_err(|e| Error::Network(e.to_string()))
    }
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
        parts.push("触发速率限制，请稍后重试（客户端会在可解析 reset 时自动等待）".into());
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
}

