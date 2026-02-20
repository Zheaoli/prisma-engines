use indexmap::IndexMap;
use query_core::{
    BatchDocument, BatchDocumentTransaction, QueryDocument,
    schema::{QuerySchemaRef, QueryTag},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info_span;

use super::protocol_adapter::JsonProtocolAdapter;

/// SQL comments extracted from each query in the request, paired with the QueryDocument.
pub type SqlCommentsVec = Vec<Vec<(String, String)>>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum JsonBody {
    Single(JsonSingleQuery),
    Batch(JsonBatchQuery),
}

impl JsonBody {
    /// Convert a `JsonBody` into a `QueryDocument`, also extracting per-query SQL comments.
    /// Returns `(QueryDocument, SqlCommentsVec)` where each entry in SqlCommentsVec corresponds
    /// to one query in the document (single: 1 entry, batch: N entries).
    pub fn into_doc(self, query_schema: &QuerySchemaRef) -> crate::Result<(QueryDocument, SqlCommentsVec)> {
        let _span = info_span!("prisma:engine:into_doc").entered();
        match self {
            JsonBody::Single(query) => {
                let sql_comments = extract_sql_comments(&query.sql_comments);
                let operation = JsonProtocolAdapter::new(query_schema).convert_single(query)?;

                Ok((QueryDocument::Single(operation), vec![sql_comments]))
            }
            JsonBody::Batch(query) => {
                let mut protocol_adapter = JsonProtocolAdapter::new(query_schema);
                let mut all_sql_comments = Vec::with_capacity(query.batch.len());
                let mut operations = Vec::with_capacity(query.batch.len());

                for single_query in query.batch {
                    all_sql_comments.push(extract_sql_comments(&single_query.sql_comments));
                    operations.push(protocol_adapter.convert_single(single_query)?);
                }

                let transaction = if let Some(opts) = query.transaction {
                    Some(BatchDocumentTransaction::new(opts.isolation_level))
                } else {
                    None
                };

                Ok((
                    QueryDocument::Multi(BatchDocument::new(operations, transaction)),
                    all_sql_comments,
                ))
            }
        }
    }
}

fn extract_sql_comments(comments: &Option<HashMap<String, String>>) -> Vec<(String, String)> {
    comments
        .as_ref()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonSingleQuery {
    pub model_name: Option<String>,
    pub action: Action,
    pub query: FieldQuery,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql_comments: Option<HashMap<String, String>>,
}

impl JsonSingleQuery {
    pub fn action(&self) -> &Action {
        &self.action
    }

    pub fn model(&self) -> Option<&String> {
        self.model_name.as_ref()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JsonBatchQuery {
    pub batch: Vec<JsonSingleQuery>,
    pub transaction: Option<BatchTransactionOption>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTransactionOption {
    pub isolation_level: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FieldQuery {
    pub arguments: Option<IndexMap<String, serde_json::Value>>,
    pub selection: SelectionSet,
}

#[derive(Debug)]
pub struct Action(QueryTag);

impl Action {
    pub fn new(query_tag: QueryTag) -> Self {
        Self(query_tag)
    }

    pub fn value(&self) -> QueryTag {
        self.0
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

const ALL_SCALARS: &str = "$scalars";
const ALL_COMPOSITES: &str = "$composites";

#[derive(Debug, Deserialize)]
pub struct SelectionSet(IndexMap<String, SelectionSetValue>);

impl SelectionSet {
    pub fn new(selection_set: IndexMap<String, SelectionSetValue>) -> Self {
        Self(selection_set)
    }

    pub fn is_all_scalars(key: &str) -> bool {
        key == ALL_SCALARS
    }

    pub fn all_scalars(&self) -> bool {
        self.0.contains_key(ALL_SCALARS)
    }

    pub fn all_composites(&self) -> bool {
        self.0.contains_key(ALL_COMPOSITES)
    }

    pub fn is_all_composites(key: &str) -> bool {
        key == ALL_COMPOSITES
    }

    pub fn get_excluded_keys(&self) -> Vec<String> {
        self.0
            .iter()
            .filter_map(|(k, v)| (!v.is_selected()).then_some(k.to_owned()))
            .collect()
    }

    pub(crate) fn into_selection(self) -> impl Iterator<Item = (String, SelectionSetValue)> {
        self.0.into_iter()
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum SelectionSetValue {
    Shorthand(bool),
    Nested(FieldQuery),
}

impl SelectionSetValue {
    pub fn is_selected(&self) -> bool {
        match self {
            SelectionSetValue::Shorthand(b) => *b,
            SelectionSetValue::Nested(_) => true,
        }
    }
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let action = String::deserialize(deserializer)?;
        let query_tag = QueryTag::from(action.as_str());

        Ok(Action(query_tag))
    }
}

impl Serialize for Action {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.to_string().serialize(serializer)
    }
}

impl Serialize for SelectionSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_single_query_with_sql_comments() {
        let json = r#"{
            "modelName": "User",
            "action": "findMany",
            "query": {
                "arguments": {},
                "selection": { "$scalars": true }
            },
            "sqlComments": {
                "route": "/api/users",
                "controller": "UserController"
            }
        }"#;

        let body: JsonBody = serde_json::from_str(json).unwrap();
        match body {
            JsonBody::Single(query) => {
                assert!(query.sql_comments.is_some());
                let comments = query.sql_comments.unwrap();
                assert_eq!(comments.get("route").unwrap(), "/api/users");
                assert_eq!(comments.get("controller").unwrap(), "UserController");
            }
            _ => panic!("Expected JsonBody::Single"),
        }
    }

    #[test]
    fn test_deserialize_single_query_without_sql_comments() {
        let json = r#"{
            "modelName": "User",
            "action": "findMany",
            "query": {
                "arguments": {},
                "selection": { "$scalars": true }
            }
        }"#;

        let body: JsonBody = serde_json::from_str(json).unwrap();
        match body {
            JsonBody::Single(query) => {
                assert!(query.sql_comments.is_none());
            }
            _ => panic!("Expected JsonBody::Single"),
        }
    }

    #[test]
    fn test_deserialize_batch_query_with_sql_comments() {
        let json = r#"{
            "batch": [
                {
                    "modelName": "User",
                    "action": "findMany",
                    "query": {
                        "arguments": {},
                        "selection": { "$scalars": true }
                    },
                    "sqlComments": { "route": "/api/users" }
                },
                {
                    "modelName": "Post",
                    "action": "findMany",
                    "query": {
                        "arguments": {},
                        "selection": { "$scalars": true }
                    },
                    "sqlComments": { "route": "/api/posts" }
                }
            ]
        }"#;

        let body: JsonBody = serde_json::from_str(json).unwrap();
        match body {
            JsonBody::Batch(batch) => {
                assert_eq!(batch.batch.len(), 2);
                assert_eq!(
                    batch.batch[0].sql_comments.as_ref().unwrap().get("route").unwrap(),
                    "/api/users"
                );
                assert_eq!(
                    batch.batch[1].sql_comments.as_ref().unwrap().get("route").unwrap(),
                    "/api/posts"
                );
            }
            _ => panic!("Expected JsonBody::Batch"),
        }
    }

    #[test]
    fn test_extract_sql_comments_with_values() {
        let mut map = HashMap::new();
        map.insert("route".to_string(), "/api".to_string());
        map.insert("controller".to_string(), "UserCtrl".to_string());
        let result = extract_sql_comments(&Some(map));
        assert_eq!(result.len(), 2);
        assert!(result.contains(&("route".to_string(), "/api".to_string())));
        assert!(result.contains(&("controller".to_string(), "UserCtrl".to_string())));
    }

    #[test]
    fn test_extract_sql_comments_with_none() {
        let result = extract_sql_comments(&None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_sql_comments_with_empty_map() {
        let map = HashMap::new();
        let result = extract_sql_comments(&Some(map));
        assert!(result.is_empty());
    }

    #[test]
    fn test_serialize_single_query_with_sql_comments() {
        let mut comments = HashMap::new();
        comments.insert("route".to_string(), "/api/users".to_string());

        let query = JsonSingleQuery {
            model_name: Some("User".to_string()),
            action: Action::new(QueryTag::FindMany),
            query: FieldQuery {
                arguments: None,
                selection: SelectionSet::new(IndexMap::new()),
            },
            sql_comments: Some(comments),
        };

        let json = serde_json::to_string(&query).unwrap();
        assert!(json.contains("sqlComments"));
        assert!(json.contains("/api/users"));
    }

    #[test]
    fn test_serialize_single_query_without_sql_comments_omits_field() {
        let query = JsonSingleQuery {
            model_name: Some("User".to_string()),
            action: Action::new(QueryTag::FindMany),
            query: FieldQuery {
                arguments: None,
                selection: SelectionSet::new(IndexMap::new()),
            },
            sql_comments: None,
        };

        let json = serde_json::to_string(&query).unwrap();
        assert!(!json.contains("sqlComments"));
    }
}
