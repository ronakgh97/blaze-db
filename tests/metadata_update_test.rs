use anyhow::Result;
use blaze_db::core::{SERVER_FILE, VectorBase};

#[tokio::test]
async fn test_metadata_updates() -> Result<()> {
    let temp_dir = std::env::temp_dir().join("blaze_metadata_test");
    std::fs::create_dir_all(&temp_dir)?;

    let test_source = "test_metadata_src";
    let test_db = "test_metadata_db";

    // Create a test source and database
    {
        let mut server_file = SERVER_FILE.write().await;

        // Add source
        let src_id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        server_file
            .add_source(src_id, test_source.to_string(), timestamp.clone())
            .await?;

        // Add vector base
        let vb = VectorBase::new(test_db.to_string(), 1024, "COSINE".to_string());
        server_file.add_vector_base(test_source, vb)?;

        println!("✓ Created test source and database");
    }

    {
        let mut server_file = SERVER_FILE.write().await;
        server_file.update_node_count(test_source, test_db, 500)?;
        println!("✓ Updated node_count to 500");
    }

    // Verify node_count update
    {
        let server_file = SERVER_FILE.read().await;
        let vb = server_file.get_vector_base(test_source, test_db)?;
        assert!(vb.is_some());
        let vb = vb.unwrap();
        assert_eq!(vb.node_count, 500);
        println!("✓ Verified node_count = 500");
    }

    let initial_timestamp = {
        let server_file = SERVER_FILE.read().await;
        let vb = server_file.get_vector_base(test_source, test_db)?;
        vb.unwrap().last_queried_at.clone()
    };

    // Wait a bit to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    {
        let mut server_file = SERVER_FILE.write().await;
        server_file.touch_vector_base(test_source, test_db)?;
        println!("✓ Touched vector base");
    }

    // Verify last_accessed_at update
    {
        let server_file = SERVER_FILE.read().await;
        let vb = server_file.get_vector_base(test_source, test_db)?;
        assert!(vb.is_some());
        let vb = vb.unwrap();
        assert_ne!(vb.last_queried_at, initial_timestamp);
        println!(
            "✓ Verified last_accessed_at updated from {} to {}",
            initial_timestamp, vb.last_queried_at
        );
    }

    {
        let mut server_file = SERVER_FILE.write().await;
        server_file.remove_source(test_source, true).await?;
        println!("✓ Cleaned up test data");
    }

    Ok(())
}
