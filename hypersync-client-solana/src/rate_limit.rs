/// Rate limit information extracted from response headers.
///
/// Envoy's rate limiter returns these headers in the IETF draft format:
/// - `x-ratelimit-limit`: e.g. `"50, 50;w=60"` (total quota for the window)
/// - `x-ratelimit-remaining`: e.g. `"40"` (remaining budget in window)
/// - `x-ratelimit-reset`: e.g. `"41"` (seconds until window resets)
/// - `x-ratelimit-cost`: e.g. `"10"` (budget consumed per request)
///
/// This mirrors the EVM `hypersync-client` type of the same name so consumers
/// can share back-off logic across both clients.
#[derive(Debug, Clone, Default)]
pub struct RateLimitInfo {
    /// Total request quota for the current window.
    ///
    /// Parsed from `x-ratelimit-limit`. For IETF draft format like `"50, 50;w=60"`,
    /// the first integer before the comma is used.
    pub limit: Option<u64>,
    /// Remaining budget in the current window.
    ///
    /// Parsed from `x-ratelimit-remaining`. Note this is budget units, not request count.
    /// Divide by [`cost`](Self::cost) to get the number of requests remaining.
    pub remaining: Option<u64>,
    /// Seconds until the rate limit window resets.
    ///
    /// Parsed from `x-ratelimit-reset`.
    pub reset_secs: Option<u64>,
    /// Budget consumed per request.
    ///
    /// Parsed from `x-ratelimit-cost`. For example, if `limit` is 50 and `cost` is 10,
    /// you can make 5 requests per window.
    pub cost: Option<u64>,
}

impl std::fmt::Display for RateLimitInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if let (Some(remaining), Some(limit)) = (self.remaining, self.limit) {
            let cost = self.cost.filter(|cost| *cost > 0).unwrap_or(1);
            parts.push(format!(
                "remaining={}/{} reqs",
                remaining / cost,
                limit / cost,
            ));
        } else {
            if let Some(remaining) = self.remaining {
                parts.push(format!("remaining={remaining}"));
            }
            if let Some(limit) = self.limit {
                parts.push(format!("limit={limit}"));
            }
        }
        if let Some(reset) = self.reset_secs {
            parts.push(format!("resets_in={reset}s"));
        }
        write!(f, "{}", parts.join(", "))
    }
}

impl RateLimitInfo {
    /// Extracts rate limit information from HTTP response headers.
    ///
    /// All parsing is best-effort: missing or unparseable headers become `None`.
    pub(crate) fn from_response(res: &reqwest::Response) -> Self {
        Self {
            limit: Self::parse_limit_header(res),
            remaining: Self::parse_u64_header(res, "x-ratelimit-remaining"),
            reset_secs: Self::parse_u64_header(res, "x-ratelimit-reset"),
            cost: Self::parse_u64_header(res, "x-ratelimit-cost"),
        }
    }

    /// Returns `true` if the rate limit quota has been exhausted.
    pub fn is_rate_limited(&self) -> bool {
        self.remaining == Some(0)
    }

    /// Returns the suggested number of seconds to wait before making another request.
    pub fn suggested_wait_secs(&self) -> Option<u64> {
        self.reset_secs
    }

    /// Same as [`suggested_wait_secs`](Self::suggested_wait_secs), clamped to
    /// [`MAX_RATE_LIMIT_WAIT_SECS`]. This is what the client actually sleeps.
    pub fn capped_wait_secs(&self) -> Option<u64> {
        self.suggested_wait_secs()
            .map(|secs| secs.min(MAX_RATE_LIMIT_WAIT_SECS))
    }

    /// Parses `x-ratelimit-limit` which uses IETF draft format: `"60, 60;w=60"`.
    /// Extracts the first integer before the comma.
    fn parse_limit_header(res: &reqwest::Response) -> Option<u64> {
        let value = res.headers().get("x-ratelimit-limit")?.to_str().ok()?;
        // Take first value before comma: "60, 60;w=60" -> "60"
        let first = value.split(',').next()?.trim();
        first.parse().ok()
    }

    /// Parses a simple u64 header value.
    fn parse_u64_header(res: &reqwest::Response, name: &str) -> Option<u64> {
        res.headers().get(name)?.to_str().ok()?.trim().parse().ok()
    }
}

/// Upper bound for any single rate-limit sleep, in seconds.
///
/// `x-ratelimit-reset` is server-controlled and `http_req_timeout` does not
/// cover the sleep that follows a 429, so an unclamped value would stall the
/// calling task for as long as the server asks (and the retry loop repeats the
/// sleep up to `max_num_retries` times). Waits are clamped to this bound in
/// both the retry path and [`Client::wait_for_rate_limit`](crate::Client::wait_for_rate_limit);
/// a longer window simply costs an extra 429 round trip instead of a stall.
pub const MAX_RATE_LIMIT_WAIT_SECS: u64 = 60;

/// Response that includes rate limit information from the server.
///
/// Returned by [`Client::get_with_rate_limit`](crate::Client::get_with_rate_limit)
/// and [`Client::get_arrow_with_rate_limit`](crate::Client::get_arrow_with_rate_limit).
/// Use this when you need to inspect rate limit headers for external monitoring
/// or coordination across systems.
///
/// Mirrors the EVM `hypersync-client` type of the same name, field for field,
/// so consumers can share back-off logic across both clients.
#[derive(Debug, Clone)]
pub struct QueryResponseWithRateLimit<T> {
    /// The query response data.
    pub response: T,
    /// Rate limit information from response headers (if present).
    pub rate_limit: RateLimitInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_rate_limited() {
        let info = RateLimitInfo {
            remaining: Some(0),
            ..Default::default()
        };
        assert!(info.is_rate_limited());

        let info = RateLimitInfo {
            remaining: Some(5),
            ..Default::default()
        };
        assert!(!info.is_rate_limited());

        let info = RateLimitInfo::default();
        assert!(!info.is_rate_limited());
    }

    #[test]
    fn test_from_response_header_case_insensitive() {
        // Build an http::Response with mixed-case headers, then convert to reqwest::Response.
        // This confirms that HeaderMap normalizes names so our lowercase lookups match.
        let http_resp = http::Response::builder()
            .header("X-RateLimit-Remaining", "42")
            .header("X-RATELIMIT-RESET", "30")
            .header("X-Ratelimit-Limit", "100, 100;w=60")
            .header("X-Ratelimit-Cost", "10")
            .body("")
            .unwrap();
        let resp: reqwest::Response = http_resp.into();

        let info = RateLimitInfo::from_response(&resp);
        assert_eq!(
            (info.limit, info.remaining, info.reset_secs, info.cost),
            (Some(100), Some(42), Some(30), Some(10))
        );
    }

    #[test]
    fn test_suggested_wait_secs() {
        let info = RateLimitInfo {
            reset_secs: Some(30),
            ..Default::default()
        };
        assert_eq!(info.suggested_wait_secs(), Some(30));

        let info = RateLimitInfo::default();
        assert_eq!(info.suggested_wait_secs(), None);
    }

    #[test]
    fn test_capped_wait_secs() {
        let under = RateLimitInfo {
            reset_secs: Some(MAX_RATE_LIMIT_WAIT_SECS - 1),
            ..Default::default()
        };
        assert_eq!(
            under.capped_wait_secs(),
            Some(MAX_RATE_LIMIT_WAIT_SECS - 1),
            "a wait inside the bound is passed through untouched"
        );

        let hostile = RateLimitInfo {
            reset_secs: Some(u64::MAX),
            ..Default::default()
        };
        assert_eq!(
            hostile.capped_wait_secs(),
            Some(MAX_RATE_LIMIT_WAIT_SECS),
            "a server-controlled reset must never stall the caller past the bound"
        );

        assert_eq!(RateLimitInfo::default().capped_wait_secs(), None);
    }

    #[test]
    fn test_display_full() {
        let info = RateLimitInfo {
            limit: Some(50),
            remaining: Some(0),
            reset_secs: Some(59),
            cost: Some(10),
        };
        assert_eq!(info.to_string(), "remaining=0/5 reqs, resets_in=59s");
    }

    #[test]
    fn test_display_partial() {
        let info = RateLimitInfo {
            remaining: Some(3),
            reset_secs: Some(30),
            ..Default::default()
        };
        assert_eq!(info.to_string(), "remaining=3, resets_in=30s");
    }

    #[test]
    fn test_display_empty() {
        let info = RateLimitInfo::default();
        assert_eq!(info.to_string(), "");
    }

    #[test]
    fn test_display_zero_cost_does_not_panic() {
        let info = RateLimitInfo {
            limit: Some(50),
            remaining: Some(0),
            cost: Some(0),
            ..Default::default()
        };
        assert_eq!(info.to_string(), "remaining=0/50 reqs");
    }
}
