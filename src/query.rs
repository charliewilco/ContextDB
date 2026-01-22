use crate::types::Entry;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A unified query that can combine semantic, textual, graph, and temporal operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    /// Semantic similarity search (vector-based)
    pub meaning: Option<MeaningFilter>,
    
    /// Text-based search on expression field
    pub expression: Option<ExpressionFilter>,
    
    /// Metadata-based filters
    pub context: Option<ContextFilter>,
    
    /// Graph-based relationship queries
    pub relations: Option<RelationFilter>,
    
    /// Temporal filters
    pub temporal: Option<TemporalFilter>,
    
    /// Maximum number of results to return
    pub limit: Option<usize>,
    
    /// Whether to explain why results matched
    pub explain: bool,
}

/// Semantic similarity search parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeaningFilter {
    /// The query vector to compare against
    pub vector: Vec<f32>,
    
    /// Minimum similarity threshold (0.0 to 1.0)
    pub threshold: Option<f32>,
    
    /// Maximum number of results from vector search
    pub top_k: Option<usize>,
}

/// Text-based search on the expression field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpressionFilter {
    /// Exact match
    Equals(String),
    
    /// Contains substring (case-insensitive)
    Contains(String),
    
    /// Starts with prefix
    StartsWith(String),
    
    /// Regex match
    Matches(String),
}

/// Filter based on context metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextFilter {
    /// Check if a JSON path exists
    PathExists(String),
    
    /// Check if a JSON path equals a value
    PathEquals(String, serde_json::Value),
    
    /// Check if a JSON path contains a value (for arrays)
    PathContains(String, serde_json::Value),
    
    /// Combine multiple filters with AND
    And(Vec<ContextFilter>),
    
    /// Combine multiple filters with OR
    Or(Vec<ContextFilter>),
}

/// Graph-based relationship queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationFilter {
    /// Entries directly related to this ID
    DirectlyRelatedTo(Uuid),
    
    /// Entries within N hops of this ID
    WithinDistance { from: Uuid, max_hops: usize },
    
    /// Entries that have any relations
    HasRelations,
    
    /// Entries that have no relations
    NoRelations,
}

/// Temporal filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemporalFilter {
    /// Created after this time
    CreatedAfter(DateTime<Utc>),
    
    /// Created before this time
    CreatedBefore(DateTime<Utc>),
    
    /// Created between these times
    CreatedBetween(DateTime<Utc>, DateTime<Utc>),
    
    /// Updated after this time
    UpdatedAfter(DateTime<Utc>),
    
    /// Updated before this time
    UpdatedBefore(DateTime<Utc>),
}

/// Result of a query with optional explanation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// The matching entry
    pub entry: Entry,
    
    /// Similarity score if semantic search was used
    pub similarity_score: Option<f32>,
    
    /// Explanation of why this entry matched (if requested)
    pub explanation: Option<String>,
}

impl Query {
    /// Create a new empty query
    pub fn new() -> Self {
        Self {
            meaning: None,
            expression: None,
            context: None,
            relations: None,
            temporal: None,
            limit: None,
            explain: false,
        }
    }
    
    /// Add semantic search by vector similarity
    pub fn with_meaning(mut self, vector: Vec<f32>, threshold: Option<f32>) -> Self {
        self.meaning = Some(MeaningFilter {
            vector,
            threshold,
            top_k: None,
        });
        self
    }
    
    /// Add text search on expression
    pub fn with_expression(mut self, filter: ExpressionFilter) -> Self {
        self.expression = Some(filter);
        self
    }
    
    /// Add context metadata filter
    pub fn with_context(mut self, filter: ContextFilter) -> Self {
        self.context = Some(filter);
        self
    }
    
    /// Add temporal filter
    pub fn with_temporal(mut self, filter: TemporalFilter) -> Self {
        self.temporal = Some(filter);
        self
    }
    
    /// Limit number of results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
    
    /// Enable explanations
    pub fn with_explanation(mut self) -> Self {
        self.explain = true;
        self
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // ==================== Query Builder Tests ====================

    #[test]
    fn test_query_new_is_empty() {
        let query = Query::new();

        assert!(query.meaning.is_none());
        assert!(query.expression.is_none());
        assert!(query.context.is_none());
        assert!(query.relations.is_none());
        assert!(query.temporal.is_none());
        assert!(query.limit.is_none());
        assert!(!query.explain);
    }

    #[test]
    fn test_query_default_equals_new() {
        let query_new = Query::new();
        let query_default = Query::default();

        assert!(query_new.meaning.is_none() && query_default.meaning.is_none());
        assert!(query_new.expression.is_none() && query_default.expression.is_none());
        assert!(query_new.limit.is_none() && query_default.limit.is_none());
        assert_eq!(query_new.explain, query_default.explain);
    }

    #[test]
    fn test_query_with_meaning() {
        let vector = vec![0.1, 0.2, 0.3];
        let query = Query::new().with_meaning(vector.clone(), Some(0.8));

        let meaning = query.meaning.unwrap();
        assert_eq!(meaning.vector, vector);
        assert_eq!(meaning.threshold, Some(0.8));
        assert!(meaning.top_k.is_none());
    }

    #[test]
    fn test_query_with_meaning_no_threshold() {
        let vector = vec![0.1, 0.2, 0.3];
        let query = Query::new().with_meaning(vector.clone(), None);

        let meaning = query.meaning.unwrap();
        assert_eq!(meaning.vector, vector);
        assert!(meaning.threshold.is_none());
    }

    #[test]
    fn test_query_with_expression_equals() {
        let query = Query::new()
            .with_expression(ExpressionFilter::Equals("test".to_string()));

        match query.expression.unwrap() {
            ExpressionFilter::Equals(s) => assert_eq!(s, "test"),
            _ => panic!("Expected Equals filter"),
        }
    }

    #[test]
    fn test_query_with_expression_contains() {
        let query = Query::new()
            .with_expression(ExpressionFilter::Contains("test".to_string()));

        match query.expression.unwrap() {
            ExpressionFilter::Contains(s) => assert_eq!(s, "test"),
            _ => panic!("Expected Contains filter"),
        }
    }

    #[test]
    fn test_query_with_expression_starts_with() {
        let query = Query::new()
            .with_expression(ExpressionFilter::StartsWith("prefix".to_string()));

        match query.expression.unwrap() {
            ExpressionFilter::StartsWith(s) => assert_eq!(s, "prefix"),
            _ => panic!("Expected StartsWith filter"),
        }
    }

    #[test]
    fn test_query_with_expression_matches() {
        let query = Query::new()
            .with_expression(ExpressionFilter::Matches("pattern".to_string()));

        match query.expression.unwrap() {
            ExpressionFilter::Matches(s) => assert_eq!(s, "pattern"),
            _ => panic!("Expected Matches filter"),
        }
    }

    #[test]
    fn test_query_with_context_path_exists() {
        let query = Query::new()
            .with_context(ContextFilter::PathExists("/foo/bar".to_string()));

        match query.context.unwrap() {
            ContextFilter::PathExists(path) => assert_eq!(path, "/foo/bar"),
            _ => panic!("Expected PathExists filter"),
        }
    }

    #[test]
    fn test_query_with_context_path_equals() {
        let value = serde_json::json!("test_value");
        let query = Query::new()
            .with_context(ContextFilter::PathEquals("/key".to_string(), value.clone()));

        match query.context.unwrap() {
            ContextFilter::PathEquals(path, v) => {
                assert_eq!(path, "/key");
                assert_eq!(v, value);
            }
            _ => panic!("Expected PathEquals filter"),
        }
    }

    #[test]
    fn test_query_with_context_path_contains() {
        let value = serde_json::json!("item");
        let query = Query::new()
            .with_context(ContextFilter::PathContains("/array".to_string(), value.clone()));

        match query.context.unwrap() {
            ContextFilter::PathContains(path, v) => {
                assert_eq!(path, "/array");
                assert_eq!(v, value);
            }
            _ => panic!("Expected PathContains filter"),
        }
    }

    #[test]
    fn test_query_with_context_and() {
        let filter = ContextFilter::And(vec![
            ContextFilter::PathExists("/a".to_string()),
            ContextFilter::PathExists("/b".to_string()),
        ]);
        let query = Query::new().with_context(filter);

        match query.context.unwrap() {
            ContextFilter::And(filters) => assert_eq!(filters.len(), 2),
            _ => panic!("Expected And filter"),
        }
    }

    #[test]
    fn test_query_with_context_or() {
        let filter = ContextFilter::Or(vec![
            ContextFilter::PathExists("/a".to_string()),
            ContextFilter::PathExists("/b".to_string()),
        ]);
        let query = Query::new().with_context(filter);

        match query.context.unwrap() {
            ContextFilter::Or(filters) => assert_eq!(filters.len(), 2),
            _ => panic!("Expected Or filter"),
        }
    }

    #[test]
    fn test_query_with_temporal_created_after() {
        let dt = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let query = Query::new()
            .with_temporal(TemporalFilter::CreatedAfter(dt));

        match query.temporal.unwrap() {
            TemporalFilter::CreatedAfter(d) => assert_eq!(d, dt),
            _ => panic!("Expected CreatedAfter filter"),
        }
    }

    #[test]
    fn test_query_with_temporal_created_before() {
        let dt = Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 59).unwrap();
        let query = Query::new()
            .with_temporal(TemporalFilter::CreatedBefore(dt));

        match query.temporal.unwrap() {
            TemporalFilter::CreatedBefore(d) => assert_eq!(d, dt),
            _ => panic!("Expected CreatedBefore filter"),
        }
    }

    #[test]
    fn test_query_with_temporal_created_between() {
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 59).unwrap();
        let query = Query::new()
            .with_temporal(TemporalFilter::CreatedBetween(start, end));

        match query.temporal.unwrap() {
            TemporalFilter::CreatedBetween(s, e) => {
                assert_eq!(s, start);
                assert_eq!(e, end);
            }
            _ => panic!("Expected CreatedBetween filter"),
        }
    }

    #[test]
    fn test_query_with_temporal_updated_after() {
        let dt = Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let query = Query::new()
            .with_temporal(TemporalFilter::UpdatedAfter(dt));

        match query.temporal.unwrap() {
            TemporalFilter::UpdatedAfter(d) => assert_eq!(d, dt),
            _ => panic!("Expected UpdatedAfter filter"),
        }
    }

    #[test]
    fn test_query_with_temporal_updated_before() {
        let dt = Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let query = Query::new()
            .with_temporal(TemporalFilter::UpdatedBefore(dt));

        match query.temporal.unwrap() {
            TemporalFilter::UpdatedBefore(d) => assert_eq!(d, dt),
            _ => panic!("Expected UpdatedBefore filter"),
        }
    }

    #[test]
    fn test_query_with_limit() {
        let query = Query::new().with_limit(10);
        assert_eq!(query.limit, Some(10));
    }

    #[test]
    fn test_query_with_limit_zero() {
        let query = Query::new().with_limit(0);
        assert_eq!(query.limit, Some(0));
    }

    #[test]
    fn test_query_with_explanation() {
        let query = Query::new().with_explanation();
        assert!(query.explain);
    }

    #[test]
    fn test_query_builder_chain() {
        let vector = vec![0.1, 0.2, 0.3];
        let query = Query::new()
            .with_meaning(vector.clone(), Some(0.8))
            .with_expression(ExpressionFilter::Contains("test".to_string()))
            .with_limit(5)
            .with_explanation();

        assert!(query.meaning.is_some());
        assert!(query.expression.is_some());
        assert_eq!(query.limit, Some(5));
        assert!(query.explain);
    }

    #[test]
    fn test_query_all_filters_combined() {
        let vector = vec![0.1, 0.2, 0.3];
        let dt = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        let query = Query::new()
            .with_meaning(vector, Some(0.5))
            .with_expression(ExpressionFilter::Contains("test".to_string()))
            .with_context(ContextFilter::PathExists("/meta".to_string()))
            .with_temporal(TemporalFilter::CreatedAfter(dt))
            .with_limit(100)
            .with_explanation();

        assert!(query.meaning.is_some());
        assert!(query.expression.is_some());
        assert!(query.context.is_some());
        assert!(query.temporal.is_some());
        assert_eq!(query.limit, Some(100));
        assert!(query.explain);
    }

    // ==================== MeaningFilter Tests ====================

    #[test]
    fn test_meaning_filter_fields() {
        let filter = MeaningFilter {
            vector: vec![1.0, 2.0, 3.0],
            threshold: Some(0.75),
            top_k: Some(10),
        };

        assert_eq!(filter.vector.len(), 3);
        assert_eq!(filter.threshold, Some(0.75));
        assert_eq!(filter.top_k, Some(10));
    }

    #[test]
    fn test_meaning_filter_empty_vector() {
        let filter = MeaningFilter {
            vector: vec![],
            threshold: None,
            top_k: None,
        };

        assert!(filter.vector.is_empty());
    }

    // ==================== QueryResult Tests ====================

    #[test]
    fn test_query_result_fields() {
        let entry = Entry::new(vec![0.1, 0.2], "Test".to_string());
        let result = QueryResult {
            entry: entry.clone(),
            similarity_score: Some(0.95),
            explanation: Some("Matched by semantic search".to_string()),
        };

        assert_eq!(result.entry.id, entry.id);
        assert_eq!(result.similarity_score, Some(0.95));
        assert!(result.explanation.is_some());
    }

    #[test]
    fn test_query_result_no_similarity() {
        let entry = Entry::new(vec![0.1, 0.2], "Test".to_string());
        let result = QueryResult {
            entry,
            similarity_score: None,
            explanation: None,
        };

        assert!(result.similarity_score.is_none());
        assert!(result.explanation.is_none());
    }

    // ==================== Serialization Tests ====================

    #[test]
    fn test_query_serialization_roundtrip() {
        let query = Query::new()
            .with_meaning(vec![0.1, 0.2, 0.3], Some(0.8))
            .with_expression(ExpressionFilter::Contains("test".to_string()))
            .with_limit(10);

        let json = serde_json::to_string(&query).unwrap();
        let deserialized: Query = serde_json::from_str(&json).unwrap();

        assert!(deserialized.meaning.is_some());
        assert!(deserialized.expression.is_some());
        assert_eq!(deserialized.limit, Some(10));
    }

    #[test]
    fn test_expression_filter_serialization() {
        let filters = vec![
            ExpressionFilter::Equals("exact".to_string()),
            ExpressionFilter::Contains("partial".to_string()),
            ExpressionFilter::StartsWith("prefix".to_string()),
            ExpressionFilter::Matches("pattern".to_string()),
        ];

        for filter in filters {
            let json = serde_json::to_string(&filter).unwrap();
            let _deserialized: ExpressionFilter = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn test_context_filter_serialization() {
        let filter = ContextFilter::And(vec![
            ContextFilter::PathExists("/a".to_string()),
            ContextFilter::Or(vec![
                ContextFilter::PathEquals("/b".to_string(), serde_json::json!(1)),
                ContextFilter::PathContains("/c".to_string(), serde_json::json!("x")),
            ]),
        ]);

        let json = serde_json::to_string(&filter).unwrap();
        let _deserialized: ContextFilter = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_relation_filter_serialization() {
        let id = Uuid::new_v4();
        let filters = vec![
            RelationFilter::DirectlyRelatedTo(id),
            RelationFilter::WithinDistance { from: id, max_hops: 3 },
            RelationFilter::HasRelations,
            RelationFilter::NoRelations,
        ];

        for filter in filters {
            let json = serde_json::to_string(&filter).unwrap();
            let _deserialized: RelationFilter = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn test_temporal_filter_serialization() {
        let dt = Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let filters = vec![
            TemporalFilter::CreatedAfter(dt),
            TemporalFilter::CreatedBefore(dt),
            TemporalFilter::CreatedBetween(dt, dt),
            TemporalFilter::UpdatedAfter(dt),
            TemporalFilter::UpdatedBefore(dt),
        ];

        for filter in filters {
            let json = serde_json::to_string(&filter).unwrap();
            let _deserialized: TemporalFilter = serde_json::from_str(&json).unwrap();
        }
    }
}
