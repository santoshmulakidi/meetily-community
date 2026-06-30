//! Semantic Search Service

mod semantic_search_service;

pub use semantic_search_service::{
    SemanticSearchService,
    SearchRequest,
    SearchFilters,
    SearchResponse,
    SearchResult,
    QueryIntent,
};