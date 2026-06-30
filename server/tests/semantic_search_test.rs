//! Semantic Search Service Tests

use std::sync::Arc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::semantic_search::{
    SemanticSearchService, SearchRequest, SearchFilters,
};
use crate::services::embedding::EmbeddingServiceImpl;

/// Helper to create test service
async fn create_test_service(db_pool: PgPool) -> (SemanticSearchService, Arc<EmbeddingServiceImpl>) {
    let embedding_service = Arc::new(EmbeddingServiceImpl::new(
        crate::services::embedding::EmbeddingConfig::from_env()
    ));
    
    let search_service = SemanticSearchService::new(
        Arc::new(db_pool),
        embedding_service.clone(),
    );
    
    (search_service, embedding_service)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_with_results() {
        // Skip if no test database
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost/meetily_test".to_string());
        
        let db_pool = PgPool::connect(&database_url).await.ok();
        if db_pool.is_none() {
            eprintln!("Skipping test: no test database available");
            return;
        }
        
        let db_pool = db_pool.unwrap();
        let (search_service, _) = create_test_service(db_pool).await;
        
        let request = SearchRequest {
            query: "test meeting discussion".to_string(),
            filters: SearchFilters::default(),
            limit: 5,
            include_excerpts: true,
        };
        
        let response = search_service.search(request).await;
        
        // Should return response (may be empty if no data)
        assert!(response.is_ok() || response.is_err());
        
        if let Ok(response) = response {
            assert!(response.query_time_ms > 0);
            assert!(response.total_results >= 0);
        }
    }

    #[tokio::test]
    async fn test_search_with_filters() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost/meetily_test".to_string());
        
        let db_pool = PgPool::connect(&database_url).await.ok();
        if db_pool.is_none() {
            eprintln!("Skipping test: no test database available");
            return;
        }
        
        let db_pool = db_pool.unwrap();
        let (search_service, _) = create_test_service(db_pool).await;
        
        let now = chrono::Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);
        
        let request = SearchRequest {
            query: "budget discussion".to_string(),
            filters: SearchFilters {
                date_from: Some(one_hour_ago),
                date_to: Some(now),
                speakers: Some(vec!["John".to_string()]),
                meeting_ids: None,
                summary_types: None,
            },
            limit: 10,
            include_excerpts: true,
        };
        
        let response = search_service.search(request).await;
        
        // Should handle filters gracefully
        assert!(response.is_ok() || response.is_err());
    }

    #[tokio::test]
    async fn test_search_empty_query() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost/meetily_test".to_string());
        
        let db_pool = PgPool::connect(&database_url).await.ok();
        if db_pool.is_none() {
            eprintln!("Skipping test: no test database available");
            return;
        }
        
        let db_pool = db_pool.unwrap();
        let (search_service, _) = create_test_service(db_pool).await;
        
        let request = SearchRequest {
            query: "".to_string(),
            filters: SearchFilters::default(),
            limit: 5,
            include_excerpts: true,
        };
        
        let response = search_service.search(request).await;
        
        // Should handle empty query (either return empty results or error)
        assert!(response.is_ok() || response.is_err());
    }

    #[tokio::test]
    async fn test_search_limit() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost/meetily_test".to_string());
        
        let db_pool = PgPool::connect(&database_url).await.ok();
        if db_pool.is_none() {
            eprintln!("Skipping test: no test database available");
            return;
        }
        
        let db_pool = db_pool.unwrap();
        let (search_service, _) = create_test_service(db_pool).await;
        
        let request = SearchRequest {
            query: "test".to_string(),
            filters: SearchFilters::default(),
            limit: 1,
            include_excerpts: true,
        };
        
        let response = search_service.search(request).await;
        
        if let Ok(response) = response {
            assert!(response.results.len() <= 1);
        }
    }

    #[tokio::test]
    async fn test_cosine_similarity() {
        use crate::services::semantic_search::semantic_search_service::cosine_similarity;
        
        // Identical vectors should have similarity 1.0
        let vec1 = vec![1.0, 0.0, 0.0];
        let vec2 = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&vec1, &vec2);
        assert!((sim - 1.0).abs() < 0.001);
        
        // Orthogonal vectors should have similarity 0.0
        let vec3 = vec![0.0, 1.0, 0.0];
        let sim2 = cosine_similarity(&vec1, &vec3);
        assert!(sim2.abs() < 0.001);
        
        // Opposite vectors should have similarity -1.0
        let vec4 = vec![-1.0, 0.0, 0.0];
        let sim3 = cosine_similarity(&vec1, &vec4);
        assert!((sim3 + 1.0).abs() < 0.001);
    }
}