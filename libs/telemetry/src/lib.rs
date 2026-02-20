pub mod collector;
pub mod exporter;
pub mod filter;
pub mod formatting;
pub mod id;
pub mod layer;
pub mod models;
pub mod time;
pub mod traceparent;

pub use exporter::Exporter;
pub use id::{NextId, RequestId};
pub use layer::layer;
pub use traceparent::TraceParent;

/// Bundles trace information for SQL query comments.
/// Carries both the W3C traceparent and user-provided SQL comments.
#[derive(Debug, Clone, Default)]
pub struct SqlTrace {
    pub traceparent: Option<TraceParent>,
    pub sql_comments: Vec<(String, String)>,
}

impl SqlTrace {
    pub fn new(traceparent: Option<TraceParent>, sql_comments: Vec<(String, String)>) -> Self {
        Self {
            traceparent,
            sql_comments,
        }
    }

    pub fn from_traceparent(traceparent: Option<TraceParent>) -> Self {
        Self {
            traceparent,
            sql_comments: Vec::new(),
        }
    }
}

#[cfg(test)]
mod sql_trace_tests {
    use super::*;

    #[test]
    fn test_default_sql_trace() {
        let trace = SqlTrace::default();
        assert!(trace.traceparent.is_none());
        assert!(trace.sql_comments.is_empty());
    }

    #[test]
    fn test_new_with_traceparent_and_comments() {
        let tp: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            .parse()
            .unwrap();
        let comments = vec![("route".to_string(), "/api".to_string())];
        let trace = SqlTrace::new(Some(tp), comments.clone());
        assert_eq!(trace.traceparent, Some(tp));
        assert_eq!(trace.sql_comments, comments);
    }

    #[test]
    fn test_from_traceparent() {
        let tp: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            .parse()
            .unwrap();
        let trace = SqlTrace::from_traceparent(Some(tp));
        assert_eq!(trace.traceparent, Some(tp));
        assert!(trace.sql_comments.is_empty());
    }

    #[test]
    fn test_from_traceparent_none() {
        let trace = SqlTrace::from_traceparent(None);
        assert!(trace.traceparent.is_none());
        assert!(trace.sql_comments.is_empty());
    }
}
