use telemetry::{TraceParent, SqlTrace};

/// Per-query context carrying trace information and SQL comments.
/// This replaces the standalone `traceparent: Option<TraceParent>` parameter
/// that was previously threaded through the query execution pipeline.
#[derive(Debug, Clone, Default)]
pub struct QueryContext {
    pub traceparent: Option<TraceParent>,
    pub sql_comments: Vec<(String, String)>,
}

impl QueryContext {
    pub fn new(traceparent: Option<TraceParent>, sql_comments: Vec<(String, String)>) -> Self {
        Self {
            traceparent,
            sql_comments,
        }
    }

    pub fn with_traceparent(traceparent: Option<TraceParent>) -> Self {
        Self {
            traceparent,
            sql_comments: Vec::new(),
        }
    }

    pub fn with_sql_comments(&self, sql_comments: Vec<(String, String)>) -> Self {
        Self {
            traceparent: self.traceparent,
            sql_comments,
        }
    }

    pub fn sql_trace(&self) -> SqlTrace {
        SqlTrace::new(self.traceparent, self.sql_comments.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_query_context() {
        let ctx = QueryContext::default();
        assert!(ctx.traceparent.is_none());
        assert!(ctx.sql_comments.is_empty());
    }

    #[test]
    fn test_new_with_comments() {
        let comments = vec![("route".to_string(), "/api".to_string())];
        let ctx = QueryContext::new(None, comments.clone());
        assert!(ctx.traceparent.is_none());
        assert_eq!(ctx.sql_comments, comments);
    }

    #[test]
    fn test_with_traceparent() {
        let tp: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            .parse()
            .unwrap();
        let ctx = QueryContext::with_traceparent(Some(tp));
        assert_eq!(ctx.traceparent, Some(tp));
        assert!(ctx.sql_comments.is_empty());
    }

    #[test]
    fn test_with_sql_comments() {
        let tp: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            .parse()
            .unwrap();
        let ctx = QueryContext::with_traceparent(Some(tp));
        let comments = vec![("key".to_string(), "value".to_string())];
        let ctx2 = ctx.with_sql_comments(comments.clone());
        assert_eq!(ctx2.traceparent, Some(tp));
        assert_eq!(ctx2.sql_comments, comments);
    }

    #[test]
    fn test_sql_trace_conversion() {
        let tp: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            .parse()
            .unwrap();
        let comments = vec![("route".to_string(), "/api".to_string())];
        let ctx = QueryContext::new(Some(tp), comments.clone());
        let trace = ctx.sql_trace();
        assert_eq!(trace.traceparent, Some(tp));
        assert_eq!(trace.sql_comments, comments);
    }
}
