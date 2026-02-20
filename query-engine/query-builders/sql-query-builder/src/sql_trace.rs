use telemetry::TraceParent;

use crate::Context;
use quaint::ast::{Delete, Insert, Select, Update};

fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ' ' => result.push_str("%20"),
            '\'' => result.push_str("%27"),
            '/' => result.push_str("%2F"),
            '%' => result.push_str("%25"),
            _ => result.push(c),
        }
    }
    result
}

/// Build the sqlcommenter comment string from sql_comments and an optional traceparent.
/// Keys are sorted alphabetically, values are URL-encoded and single-quoted.
/// Returns `None` if there are no comments and traceparent is not sampled.
fn build_trace_comment(sql_comments: &[(String, String)], traceparent: Option<TraceParent>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    // Add user-provided sql_comments, sorted alphabetically by key
    let mut sorted_comments: Vec<_> = sql_comments.iter().collect();
    sorted_comments.sort_by_key(|(k, _)| k.as_str());
    for (key, value) in sorted_comments {
        parts.push(format!("{}='{}'", url_encode(key), url_encode(value)));
    }

    // Add traceparent if sampled
    if let Some(tp) = traceparent {
        if tp.sampled() {
            parts.push(format!("traceparent='{tp}'"));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

pub trait SqlTraceComment: Sized {
    fn add_trace_id(self, ctx: &Context<'_>) -> Self;
}

macro_rules! sql_trace {
    ($what:ty) => {
        impl SqlTraceComment for $what {
            fn add_trace_id(self, ctx: &Context<'_>) -> Self {
                match build_trace_comment(ctx.sql_comments(), ctx.traceparent()) {
                    Some(comment) => self.comment(comment),
                    None => self,
                }
            }
        }
    };
}

sql_trace!(Insert<'_>);

sql_trace!(Update<'_>);

sql_trace!(Delete<'_>);

sql_trace!(Select<'_>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encoding() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("/api/users"), "%2Fapi%2Fusers");
        assert_eq!(url_encode("it's"), "it%27s");
        assert_eq!(url_encode("100%"), "100%25");
        assert_eq!(url_encode("plain"), "plain");
    }

    #[test]
    fn test_build_trace_comment_with_sql_comments_only() {
        let comments = vec![
            ("route".to_string(), "/api/users".to_string()),
            ("controller".to_string(), "UserController".to_string()),
        ];
        let result = build_trace_comment(&comments, None).unwrap();
        // Keys should be sorted alphabetically
        assert_eq!(result, "controller='UserController',route='%2Fapi%2Fusers'");
    }

    #[test]
    fn test_build_trace_comment_with_traceparent_only() {
        let tp: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            .parse()
            .unwrap();
        let result = build_trace_comment(&[], Some(tp)).unwrap();
        assert_eq!(
            result,
            "traceparent='00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01'"
        );
    }

    #[test]
    fn test_build_trace_comment_with_both() {
        let comments = vec![
            ("route".to_string(), "/api/users".to_string()),
            ("controller".to_string(), "UserController".to_string()),
        ];
        let tp: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            .parse()
            .unwrap();
        let result = build_trace_comment(&comments, Some(tp)).unwrap();
        // sql_comments first (sorted), then traceparent
        assert_eq!(
            result,
            "controller='UserController',route='%2Fapi%2Fusers',traceparent='00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01'"
        );
    }

    #[test]
    fn test_build_trace_comment_empty_comments_unsampled_traceparent() {
        // traceparent with flags=00 means not sampled
        let tp: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00"
            .parse()
            .unwrap();
        let result = build_trace_comment(&[], Some(tp));
        assert!(result.is_none());
    }

    #[test]
    fn test_build_trace_comment_no_comments_no_traceparent() {
        let result = build_trace_comment(&[], None);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_trace_comment_url_encodes_special_chars() {
        let comments = vec![
            ("path".to_string(), "/api/user's data".to_string()),
            ("percent".to_string(), "100% done".to_string()),
        ];
        let result = build_trace_comment(&comments, None).unwrap();
        assert_eq!(
            result,
            "path='%2Fapi%2Fuser%27s%20data',percent='100%25%20done'"
        );
    }

    #[test]
    fn test_build_trace_comment_single_comment() {
        let comments = vec![("route".to_string(), "/test".to_string())];
        let result = build_trace_comment(&comments, None).unwrap();
        assert_eq!(result, "route='%2Ftest'");
    }

    #[test]
    fn test_build_trace_comment_sql_comments_with_unsampled_traceparent() {
        // Even with unsampled traceparent, sql_comments should still appear
        let comments = vec![("route".to_string(), "/api".to_string())];
        let tp: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00"
            .parse()
            .unwrap();
        let result = build_trace_comment(&comments, Some(tp)).unwrap();
        assert_eq!(result, "route='%2Fapi'");
    }
}
