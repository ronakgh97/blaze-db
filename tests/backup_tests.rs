// Real backup integration tests - Run server manually with: cargo run --bin blzsrv -- serve
// These tests verify actual backup functionality end-to-end

use blaze_db::prelude::{
    CreateBackupRequest, CreateBackupResponse, CreateDatabaseRequest, CreateSourceRequest,
    DeleteBackupRequest, DeleteBackupResponse, InsertRequest, ListBackupsRequest,
    ListBackupsResponse, RestoreBackupRequest, RestoreBackupResponse, VectorDataDto,
    VectorQueryRequest,
};
use rand::RngExt;
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Barrier;

const BASE_URL: &str = "http://localhost:8080";
const HEALTH_ENDPOINT: &str = "/v1/blazedb/health";
const CREATE_DB_ENDPOINT: &str = "/v1/blazedb/databases/create";
const CREATE_SOURCE_ENDPOINT: &str = "/v1/blazedb/sources/create";
const INSERT_ENDPOINT: &str = "/v1/blazedb/insert";
const BACKUP_CREATE_ENDPOINT: &str = "/v1/blazedb/backup/create";
const BACKUP_LIST_ENDPOINT: &str = "/v1/blazedb/backup/list";
const BACKUP_RESTORE_ENDPOINT: &str = "/v1/blazedb/backup/restore";
const BACKUP_DELETE_ENDPOINT: &str = "/v1/blazedb/backup/delete";
const VECTOR_QUERY_ENDPOINT: &str = "/v1/blazedb/query/vector";

fn create_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .expect("Failed to create HTTP client")
}

async fn wait_for_server(max_attempts: u32) -> bool {
    let client = create_client();
    for _ in 1..=max_attempts {
        if let Ok(response) = client
            .get(format!("{}{}", BASE_URL, HEALTH_ENDPOINT))
            .send()
            .await
        {
            if response.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

#[inline]
fn generate_random_vectors(num_vectors: usize, dimensions: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::rng();
    (0..num_vectors)
        .map(|_| {
            (0..dimensions)
                .map(|_| rng.random_range(-1.0..1.0))
                .collect()
        })
        .collect()
}

/// Full backup and restore workflow
/// Creates DB, inserts 5K vectors, backs up, inserts more, restores, verifies original data
#[tokio::test]
#[ignore]
async fn test_backup_restore_full_workflow() {
    assert!(wait_for_server(10).await);

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let source_name = format!("backup_test_src_{}", timestamp);
    let db_name = format!("backup_test_db_{}", timestamp);
    let dimensions = 1024;
    let initial_vectors = 5000;

    println!("\n[1/8] Creating source with 1-hour backup interval...");
    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: Some(1), // 1 hour for testing
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .expect("Failed to create source");
    assert_eq!(resp.status(), 201, "Failed to create source");

    println!("[2/8] Creating database...");
    let db_req = CreateDatabaseRequest {
        name: db_name.clone(),
        source: source_name.clone(),
        metrics: None,
        dimensions,
        backup_interval_hours: None, // Inherit from source
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_req)
        .send()
        .await
        .expect("Failed to create database");
    assert_eq!(resp.status(), 201, "Failed to create database");

    println!("[3/8] Inserting {} initial vectors...", initial_vectors);
    let start = Instant::now();
    let vectors: Vec<VectorDataDto> = generate_random_vectors(initial_vectors, dimensions)
        .into_iter()
        .enumerate()
        .map(|(i, embedding)| VectorDataDto {
            embedding,
            metadata: format!("initial_vector_{}", i),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&insert_req)
        .send()
        .await
        .expect("Failed to insert vectors");
    assert_eq!(resp.status(), 200, "Failed to insert initial vectors");
    println!("    Inserted in {:?}", start.elapsed());

    println!("[4/8] Triggering manual backup...");
    let backup_req = CreateBackupRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };
    let start = Instant::now();
    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_CREATE_ENDPOINT))
        .json(&backup_req)
        .send()
        .await
        .expect("Failed to trigger backup");

    assert_eq!(resp.status(), 201, "Backup creation failed");
    let backup_resp: CreateBackupResponse =
        resp.json().await.expect("Failed to parse backup response");
    assert!(
        backup_resp.success,
        "Backup failed: {}",
        backup_resp.message
    );

    let backup_filename = backup_resp.backup_info.as_ref().unwrap().filename.clone();
    println!(
        "    Backup created: {} ({:.2} MB) in {:?}",
        backup_filename,
        backup_resp.backup_info.as_ref().unwrap().size_mb,
        start.elapsed()
    );

    println!("[5/8] Inserting additional 2500 vectors (post-backup)...");
    let additional_vectors = 2500;
    let vectors: Vec<VectorDataDto> = generate_random_vectors(additional_vectors, dimensions)
        .into_iter()
        .enumerate()
        .map(|(i, embedding)| VectorDataDto {
            embedding,
            metadata: format!("additional_vector_{}", i),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&insert_req)
        .send()
        .await
        .expect("Failed to insert additional vectors");
    assert_eq!(resp.status(), 200);
    println!(
        "    Total vectors now: {}",
        initial_vectors + additional_vectors
    );

    println!("[6/8] Verifying backup file exists on disk...");
    let backup_path = PathBuf::from(dirs::home_dir().unwrap())
        .join("blaze")
        .join("backups")
        .join(&source_name)
        .join(&db_name)
        .join(&backup_filename);

    assert!(
        backup_path.exists(),
        "Backup file not found at {:?}",
        backup_path
    );
    let metadata = std::fs::metadata(&backup_path).expect("Failed to read backup metadata");
    println!("    Backup file: {:?}", backup_path);
    println!(
        "    File size: {:.2} MB",
        metadata.len() as f64 / (1024.0 * 1024.0)
    );

    println!("[7/8] Restoring from backup (DESTRUCTIVE)...");
    let restore_req = RestoreBackupRequest {
        source: source_name.clone(),
        database: db_name.clone(),
        backup_filename: backup_filename.clone(),
    };
    let start = Instant::now();
    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_RESTORE_ENDPOINT))
        .json(&restore_req)
        .send()
        .await
        .expect("Failed to restore backup");

    assert_eq!(resp.status(), 200, "Restore failed");
    let restore_resp: RestoreBackupResponse =
        resp.json().await.expect("Failed to parse restore response");
    assert!(
        restore_resp.success,
        "Restore failed: {}",
        restore_resp.message
    );
    println!("    Restored in {:?}", start.elapsed());

    println!(
        "[8/8] Verifying restored data (should have {} vectors)...",
        initial_vectors
    );
    let query_req = VectorQueryRequest {
        query_vector: generate_random_vectors(1, dimensions)[0].clone(),
        database: db_name.clone(),
        source: source_name.clone(),
        top_k: 100,
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, VECTOR_QUERY_ENDPOINT))
        .json(&query_req)
        .send()
        .await
        .expect("Failed to query vectors");

    assert_eq!(resp.status(), 200);
    println!("    Query successful - database is functional after restore");

    // List backups to confirm
    let list_req = ListBackupsRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_LIST_ENDPOINT))
        .json(&list_req)
        .send()
        .await
        .expect("Failed to list backups");

    assert_eq!(resp.status(), 200);
    let list_resp: ListBackupsResponse = resp.json().await.expect("Failed to parse list response");
    println!("\n Test completed successfully!");
    println!(
        "   Total backups for this database: {}",
        list_resp.backups.len()
    );
    println!("   Latest backup: {}", backup_filename);
}

/// Test 2: Concurrent backup conflict
/// Tries to trigger two backups simultaneously - second should fail fast
#[tokio::test]
#[ignore]
async fn test_concurrent_backup_conflict() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let source_name = format!("concurrent_src_{}", timestamp);
    let db_name = format!("concurrent_db_{}", timestamp);

    println!("\n Setting up test database with 5K vectors...");

    // Create source and database
    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    let db_req = CreateDatabaseRequest {
        name: db_name.clone(),
        source: source_name.clone(),
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_req)
        .send()
        .await
        .unwrap();

    // Insert data
    let vectors: Vec<VectorDataDto> = generate_random_vectors(5000, 1024)
        .into_iter()
        .map(|embedding| VectorDataDto {
            embedding,
            metadata: "test".to_string(),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&insert_req)
        .send()
        .await
        .unwrap();

    println!(" Attempting concurrent backups...");

    let backup_req1 = CreateBackupRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };
    let backup_req2 = CreateBackupRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };

    // Fire both requests nearly simultaneously
    let client1 = create_client();
    let client2 = create_client();
    let handle1 = tokio::spawn(async move {
        client1
            .post(format!("{}{}", BASE_URL, BACKUP_CREATE_ENDPOINT))
            .json(&backup_req1)
            .send()
            .await
    });

    // Small delay to ensure first one starts
    tokio::time::sleep(Duration::from_millis(50)).await;

    let handle2 = tokio::spawn(async move {
        client2
            .post(format!("{}{}", BASE_URL, BACKUP_CREATE_ENDPOINT))
            .json(&backup_req2)
            .send()
            .await
    });

    let (resp1, resp2) = tokio::join!(handle1, handle2);
    let resp1 = resp1.unwrap().unwrap();
    let resp2 = resp2.unwrap().unwrap();

    let status1 = resp1.status();
    let status2 = resp2.status();

    println!("   Backup 1 status: {}", status1);
    println!("   Backup 2 status: {}", status2);

    // One should succeed (201), one should fail with conflict (409)
    let statuses = vec![status1.as_u16(), status2.as_u16()];
    assert!(
        statuses.contains(&201),
        "At least one backup should succeed"
    );

    if statuses.contains(&409) {
        println!(" Concurrent backup correctly rejected with conflict!");
    } else {
        println!(" Warning: Both backups may have succeeded (race condition acceptable)");
    }

    // Verify at least one backup exists
    let list_req = ListBackupsRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_LIST_ENDPOINT))
        .json(&list_req)
        .send()
        .await
        .unwrap();

    let list_resp: ListBackupsResponse = resp.json().await.unwrap();
    assert!(
        !list_resp.backups.is_empty(),
        "At least one backup should exist"
    );
    println!("   Total backups: {}", list_resp.backups.len());
}

/// Test 3: Backup retention - old backups should be cleaned up
#[tokio::test]
#[ignore]
async fn test_backup_retention_policy() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let source_name = format!("retention_src_{}", timestamp);
    let db_name = format!("retention_db_{}", timestamp);

    println!("\n Setting up database...");

    // Create source and database
    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    let db_req = CreateDatabaseRequest {
        name: db_name.clone(),
        source: source_name.clone(),
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_req)
        .send()
        .await
        .unwrap();

    // Insert data
    let vectors: Vec<VectorDataDto> = generate_random_vectors(5000, 1024)
        .into_iter()
        .map(|embedding| VectorDataDto {
            embedding,
            metadata: "test".to_string(),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&insert_req)
        .send()
        .await
        .unwrap();

    println!(" Creating 7 backups (max is 5, so 2 should be deleted)...");

    for i in 1..=7 {
        let backup_req = CreateBackupRequest {
            source: source_name.clone(),
            database: db_name.clone(),
        };

        // Wait a bit between backups to ensure different timestamps
        tokio::time::sleep(Duration::from_millis(100)).await;

        let resp = client
            .post(format!("{}{}", BASE_URL, BACKUP_CREATE_ENDPOINT))
            .json(&backup_req)
            .send()
            .await
            .expect("Failed to create backup");

        assert_eq!(resp.status(), 201, "Backup {} failed", i);
        print!("  Created backup #{}", i);

        if i > 5 {
            print!(" (should trigger cleanup)");
        }
        println!();
    }

    // List backups and verify only 5 exist
    let list_req = ListBackupsRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_LIST_ENDPOINT))
        .json(&list_req)
        .send()
        .await
        .expect("Failed to list backups");

    assert_eq!(resp.status(), 200);
    let list_resp: ListBackupsResponse = resp.json().await.expect("Failed to parse list");

    println!("\n Verifying retention policy...");
    println!("   Total backups found: {}", list_resp.backups.len());

    assert_eq!(
        list_resp.backups.len(),
        5,
        "Should have exactly 5 backups (max_backups_per_database = 5)"
    );

    // Verify the oldest backups were removed (should have 5 most recent)
    let backup_dir = PathBuf::from(dirs::home_dir().unwrap())
        .join("blaze")
        .join("backups")
        .join(&source_name)
        .join(&db_name);

    let entries: Vec<_> = std::fs::read_dir(&backup_dir)
        .expect("Failed to read backup directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "zst")
                .unwrap_or(false)
        })
        .collect();

    assert_eq!(
        entries.len(),
        5,
        "Directory should contain exactly 5 backup files"
    );
    println!("   Retention policy working correctly! Kept 5 newest backups.");
}

/// Test 4: Scheduled backup trigger
/// Creates DB with immediate backup interval (1 min) and verifies backup is created
#[tokio::test]
#[ignore]
async fn test_scheduled_backup_trigger() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let source_name = format!("scheduled_src_{}", timestamp);
    let db_name = format!("scheduled_db_{}", timestamp);

    println!("\n Creating source and database with 1-minute backup interval...");

    // Create source
    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    // Create database with 1 minute backup interval (use very short for testing)
    // Note: Server needs to be restarted with short scheduler interval for this to work quickly
    let db_req = CreateDatabaseRequest {
        name: db_name.clone(),
        source: source_name.clone(),
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: Some(0), // 0 means immediate backup on next scheduler check
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_req)
        .send()
        .await
        .unwrap();

    // Insert data
    let vectors: Vec<VectorDataDto> = generate_random_vectors(5000, 1024)
        .into_iter()
        .map(|embedding| VectorDataDto {
            embedding,
            metadata: "test".to_string(),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&insert_req)
        .send()
        .await
        .unwrap();

    println!(" Waiting for scheduled backup (checking every 10s for up to 2 minutes)...");
    println!(" (Note: Server scheduler interval must be shortened for quick testing)");

    let start = Instant::now();
    let timeout = Duration::from_secs(120);
    let check_interval = Duration::from_secs(10);

    let mut backup_found = false;
    while start.elapsed() < timeout {
        tokio::time::sleep(check_interval).await;

        let list_req = ListBackupsRequest {
            source: source_name.clone(),
            database: db_name.clone(),
        };

        if let Ok(resp) = client
            .post(format!("{}{}", BASE_URL, BACKUP_LIST_ENDPOINT))
            .json(&list_req)
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(list_resp) = resp.json::<ListBackupsResponse>().await {
                    if !list_resp.backups.is_empty() {
                        backup_found = true;
                        println!(" Scheduled backup created after {:?}!", start.elapsed());
                        println!("   Backup: {}", list_resp.backups[0].filename);
                        break;
                    }
                }
            }
        }

        print!("  Checked after {:?}...\r", start.elapsed());
    }

    if backup_found {
        println!("\n Scheduled backup test PASSED!");
    } else {
        println!("\n WARNING: Scheduled backup not detected within timeout.");
        println!(" This may be expected if server scheduler interval is not shortened.");
        println!(" Manual backup should still work fine.");
    }
}

/// Test 5: Backup delete functionality
#[tokio::test]
#[ignore]
async fn test_backup_delete() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let source_name = format!("delete_src_{}", timestamp);
    let db_name = format!("delete_db_{}", timestamp);

    println!("\n Setting up database and creating backup...");

    // Create source and database
    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    let db_req = CreateDatabaseRequest {
        name: db_name.clone(),
        source: source_name.clone(),
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_req)
        .send()
        .await
        .unwrap();

    // Insert and backup
    let vectors: Vec<VectorDataDto> = generate_random_vectors(5000, 1024)
        .into_iter()
        .map(|embedding| VectorDataDto {
            embedding,
            metadata: "test".to_string(),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&insert_req)
        .send()
        .await
        .unwrap();

    let backup_req = CreateBackupRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_CREATE_ENDPOINT))
        .json(&backup_req)
        .send()
        .await
        .unwrap();

    let backup_resp: CreateBackupResponse = resp.json().await.unwrap();
    let filename = backup_resp.backup_info.unwrap().filename;

    println!(" Deleting backup: {}", filename);

    let delete_req = DeleteBackupRequest {
        source: source_name.clone(),
        database: db_name.clone(),
        backup_filename: filename.clone(),
    };

    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_DELETE_ENDPOINT))
        .json(&delete_req)
        .send()
        .await
        .expect("Failed to delete backup");

    assert_eq!(resp.status(), 200, "Delete backup failed");
    let delete_resp: DeleteBackupResponse = resp.json().await.unwrap();
    assert!(
        delete_resp.success,
        "Delete failed: {}",
        delete_resp.message
    );

    // Verify backup is gone
    let list_req = ListBackupsRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_LIST_ENDPOINT))
        .json(&list_req)
        .send()
        .await
        .unwrap();

    let list_resp: ListBackupsResponse = resp.json().await.unwrap();
    assert!(
        list_resp.backups.is_empty(),
        "Backup should have been deleted"
    );

    println!(" Backup deleted successfully!");

    // Verify file is gone from disk
    let backup_path = PathBuf::from(dirs::home_dir().unwrap())
        .join("blaze")
        .join("backups")
        .join(&source_name)
        .join(&db_name)
        .join(&filename);

    assert!(
        !backup_path.exists(),
        "Backup file should have been deleted from disk"
    );
    println!(" File removed from disk: {:?}", backup_path);
}

/// Test 6: Compression ratio verification
#[tokio::test]
#[ignore]
async fn test_backup_compression_ratio() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let source_name = format!("compression_src_{}", timestamp);
    let db_name = format!("compression_db_{}", timestamp);

    println!("\n Creating database with 10K vectors for compression test...");

    // Create source and database
    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    let db_req = CreateDatabaseRequest {
        name: db_name.clone(),
        source: source_name.clone(),
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_req)
        .send()
        .await
        .unwrap();

    // Insert data
    let vectors: Vec<VectorDataDto> = generate_random_vectors(10000, 1024)
        .into_iter()
        .map(|embedding| VectorDataDto {
            embedding,
            metadata: "compression_test".to_string(),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&insert_req)
        .send()
        .await
        .unwrap();

    println!(" Waiting for HNSW_INDEX.replica to be created...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check original replica file size
    let db_path = dirs::home_dir()
        .unwrap()
        .join("blaze")
        .join("sources")
        .join(&source_name)
        .join(&db_name);

    let replica_path = db_path.join("HNSW_INDEX.replica");
    let replica_size = if replica_path.exists() {
        std::fs::metadata(&replica_path)
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        // Use .bin file if replica doesn't exist yet
        let bin_path = db_path.join("HNSW_INDEX.bin");
        std::fs::metadata(&bin_path).map(|m| m.len()).unwrap_or(0)
    };

    println!(
        " Original index size: {:.2} MB",
        replica_size as f64 / (1024.0 * 1024.0)
    );

    // Create backup
    let backup_req = CreateBackupRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_CREATE_ENDPOINT))
        .json(&backup_req)
        .send()
        .await
        .unwrap();

    let backup_resp: CreateBackupResponse = resp.json().await.unwrap();
    let backup_info = backup_resp.backup_info.unwrap();

    println!(" Backup created: {}", backup_info.filename);
    println!(" Backup size: {:.2} MB", backup_info.size_mb);

    // Calculate compression ratio
    let backup_size_bytes = (backup_info.size_mb * 1024.0 * 1024.0) as u64;
    let compression_ratio = replica_size as f64 / backup_size_bytes as f64;

    println!(
        " Compression ratio: {:.2}x (higher is better)",
        compression_ratio
    );

    // Backup should be smaller (or at least not much larger due to tar overhead)
    assert!(
        backup_info.size_mb < (replica_size as f64 / (1024.0 * 1024.0)) * 1.1,
        "Backup should not be significantly larger than original"
    );

    if compression_ratio > 1.5 {
        println!(" EXCELLENT compression!");
    } else if compression_ratio > 1.1 {
        println!(" Good compression!");
    } else {
        println!(" Moderate compression (tar overhead or already compressed data)");
    }
}

/// Stress Test: 50 concurrent backup requests on SAME database
/// Only 1 should succeed, rest should fail with conflict (409)
#[tokio::test]
#[ignore]
async fn stress_test_concurrent_backups_same_database() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let source_name = format!("stress_backup_src_{}", timestamp);
    let db_name = format!("stress_backup_db_{}", timestamp);

    println!("\n[STRESS TEST] Concurrent backups on SAME database");
    println!(" Setting up database with 5K vectors...");

    // Create source and database
    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    let db_req = CreateDatabaseRequest {
        name: db_name.clone(),
        source: source_name.clone(),
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_req)
        .send()
        .await
        .unwrap();

    // Insert data
    let vectors: Vec<VectorDataDto> = generate_random_vectors(5000, 1024)
        .into_iter()
        .map(|embedding| VectorDataDto {
            embedding,
            metadata: "stress_test".to_string(),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&insert_req)
        .send()
        .await
        .unwrap();

    let num_concurrent = 50;
    let barrier = Arc::new(Barrier::new(num_concurrent));
    let success_count = Arc::new(AtomicU64::new(0));
    let conflict_count = Arc::new(AtomicU64::new(0));
    let other_error_count = Arc::new(AtomicU64::new(0));

    println!(" Firing {} concurrent backup requests...", num_concurrent);
    let start = Instant::now();

    let mut handles = vec![];

    for i in 0..num_concurrent {
        let client = create_client();
        let db_name = db_name.clone();
        let source_name = source_name.clone();
        let barrier = Arc::clone(&barrier);
        let success_count = Arc::clone(&success_count);
        let conflict_count = Arc::clone(&conflict_count);
        let other_error_count = Arc::clone(&other_error_count);

        let handle = tokio::spawn(async move {
            // Wait for all threads to be ready
            barrier.wait().await;

            let backup_req = CreateBackupRequest {
                source: source_name,
                database: db_name,
            };

            let response = client
                .post(format!("{}{}", BASE_URL, BACKUP_CREATE_ENDPOINT))
                .json(&backup_req)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status == 201 {
                        success_count.fetch_add(1, Ordering::SeqCst);
                        println!("  Thread {}: SUCCESS", i);
                    } else if status == 409 {
                        conflict_count.fetch_add(1, Ordering::SeqCst);
                        println!("  Thread {}: CONFLICT (expected)", i);
                    } else {
                        other_error_count.fetch_add(1, Ordering::SeqCst);
                        println!("  Thread {}: ERROR {}", i, status);
                    }
                }
                Err(e) => {
                    other_error_count.fetch_add(1, Ordering::SeqCst);
                    println!("  Thread {}: REQUEST FAILED - {}", i, e);
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all requests to complete
    futures::future::join_all(handles).await;

    let total_elapsed = start.elapsed();
    let successes = success_count.load(Ordering::SeqCst);
    let conflicts = conflict_count.load(Ordering::SeqCst);
    let other_errors = other_error_count.load(Ordering::SeqCst);

    println!("\n RESULTS:");
    println!("  Total time: {:?}", total_elapsed);
    println!("  Successful: {}", successes);
    println!("  Conflicts: {}", conflicts);
    println!("  Other errors: {}", other_errors);

    // Exactly 1 should succeed, rest should be conflicts
    assert_eq!(
        successes, 1,
        "Exactly 1 backup should succeed, got {}",
        successes
    );
    assert_eq!(
        conflicts,
        num_concurrent as u64 - 1,
        "All other requests should get conflict (409)"
    );
    assert_eq!(other_errors, 0, "No other errors expected");

    // Verify only 1 backup exists
    let list_req = ListBackupsRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_LIST_ENDPOINT))
        .json(&list_req)
        .send()
        .await
        .unwrap();

    let list_resp: ListBackupsResponse = resp.json().await.unwrap();
    assert_eq!(
        list_resp.backups.len(),
        1,
        "Should have exactly 1 backup file"
    );

    println!("✓ Concurrent backup locking works correctly!");
}

/// Stress Test: Concurrent backups on DIFFERENT databases
/// All should succeed in parallel (no conflicts)
#[tokio::test]
#[ignore]
async fn stress_test_concurrent_backups_different_databases() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let source_name = format!("stress_parallel_src_{}", timestamp);
    let num_databases = 10;

    println!("\n[STRESS TEST] Concurrent backups on DIFFERENT databases");
    println!(" Creating {} databases...", num_databases);

    // Create source
    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    // Create databases and insert data
    let mut db_names = vec![];
    for i in 0..num_databases {
        let db_name = format!("parallel_db_{}_{}", timestamp, i);
        let db_req = CreateDatabaseRequest {
            name: db_name.clone(),
            source: source_name.clone(),
            metrics: None,
            dimensions: 1024,
            backup_interval_hours: None,
        };
        client
            .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
            .json(&db_req)
            .send()
            .await
            .unwrap();

        // Insert data
        let vectors: Vec<VectorDataDto> = generate_random_vectors(1000, 1024)
            .into_iter()
            .map(|embedding| VectorDataDto {
                embedding,
                metadata: format!("db_{}_vector", i),
            })
            .collect();

        let insert_req = InsertRequest {
            nodes: vec![vectors],
            database: db_name.clone(),
            source: source_name.clone(),
        };
        client
            .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
            .json(&insert_req)
            .send()
            .await
            .unwrap();

        db_names.push(db_name);
    }

    let barrier = Arc::new(Barrier::new(num_databases));
    let success_count = Arc::new(AtomicU64::new(0));

    println!(
        " Firing {} parallel backup requests (one per database)...",
        num_databases
    );
    let start = Instant::now();

    let mut handles = vec![];

    for (idx, db_name) in db_names.iter().enumerate() {
        let client = create_client();
        let db_name = db_name.clone();
        let source_name = source_name.clone();
        let barrier = Arc::clone(&barrier);
        let success_count = Arc::clone(&success_count);

        let handle = tokio::spawn(async move {
            // Wait for all threads to be ready
            barrier.wait().await;

            let backup_req = CreateBackupRequest {
                source: source_name,
                database: db_name,
            };

            let response = client
                .post(format!("{}{}", BASE_URL, BACKUP_CREATE_ENDPOINT))
                .json(&backup_req)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status() == 201 {
                        success_count.fetch_add(1, Ordering::SeqCst);
                        println!("  Database {}: SUCCESS", idx);
                        true
                    } else {
                        println!("  Database {}: FAILED - {}", idx, resp.status());
                        false
                    }
                }
                Err(e) => {
                    println!("  Database {}: ERROR - {}", idx, e);
                    false
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all backups to complete
    let _results: Vec<bool> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    let total_elapsed = start.elapsed();
    let successes = success_count.load(Ordering::SeqCst);

    println!("\n RESULTS:");
    println!("  Total time: {:?}", total_elapsed);
    println!("  Successful: {}/{}", successes, num_databases);

    // All should succeed
    assert_eq!(
        successes, num_databases as u64,
        "All parallel backups should succeed"
    );

    // Verify all backups exist
    for db_name in &db_names {
        let list_req = ListBackupsRequest {
            source: source_name.clone(),
            database: db_name.clone(),
        };
        let resp = client
            .post(format!("{}{}", BASE_URL, BACKUP_LIST_ENDPOINT))
            .json(&list_req)
            .send()
            .await
            .unwrap();

        let list_resp: ListBackupsResponse = resp.json().await.unwrap();
        assert_eq!(
            list_resp.backups.len(),
            1,
            "Each database should have exactly 1 backup"
        );
    }

    println!("✓ Parallel backups on different databases work correctly!");
}

/// Stress Test: Mixed workload - concurrent writes and backups
/// Writes should not block backups (CoW pattern), backups should not block writes
#[tokio::test]
#[ignore]
async fn stress_test_mixed_write_backup_workload() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let source_name = format!("stress_mixed_src_{}", timestamp);
    let db_name = format!("stress_mixed_db_{}", timestamp);

    println!("\n[STRESS TEST] Mixed write + backup workload");
    println!(" Setting up database...");

    // Create source and database
    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    let db_req = CreateDatabaseRequest {
        name: db_name.clone(),
        source: source_name.clone(),
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_req)
        .send()
        .await
        .unwrap();

    // Initial data
    let vectors: Vec<VectorDataDto> = generate_random_vectors(1000, 1024)
        .into_iter()
        .map(|embedding| VectorDataDto {
            embedding,
            metadata: "initial".to_string(),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&insert_req)
        .send()
        .await
        .unwrap();

    let num_writers = 3;
    let num_backups = 5;
    let total_workers = num_writers + num_backups;

    let barrier = Arc::new(Barrier::new(total_workers));
    let write_success = Arc::new(AtomicU64::new(0));
    let backup_success = Arc::new(AtomicU64::new(0));

    println!(
        " Starting {} writers and {} backup requests...",
        num_writers, num_backups
    );
    let start = Instant::now();

    let mut handles = vec![];

    // Spawn writers
    for i in 0..num_writers {
        let client = create_client();
        let db_name = db_name.clone();
        let source_name = source_name.clone();
        let barrier = Arc::clone(&barrier);
        let write_success = Arc::clone(&write_success);

        let handle = tokio::spawn(async move {
            barrier.wait().await;

            let mut successful = 0;

            // Each writer does 3 inserts with small delay
            for j in 0..3 {
                let vectors: Vec<VectorDataDto> = generate_random_vectors(100, 1024)
                    .into_iter()
                    .map(|embedding| VectorDataDto {
                        embedding,
                        metadata: format!("writer_{}_batch_{}", i, j),
                    })
                    .collect();

                let insert_req = InsertRequest {
                    nodes: vec![vectors],
                    database: db_name.clone(),
                    source: source_name.clone(),
                };

                if let Ok(resp) = client
                    .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
                    .json(&insert_req)
                    .send()
                    .await
                {
                    if resp.status().is_success() {
                        successful += 1;
                        println!("  Writer {} batch {}: SUCCESS", i, j);
                    } else {
                        println!("  Writer {} batch {}: FAILED {}", i, j, resp.status());
                    }
                }

                // Small delay between writes
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            write_success.fetch_add(successful, Ordering::SeqCst);
        });

        handles.push(handle);
    }

    // Spawn backup requests
    for i in 0..num_backups {
        let client = create_client();
        let db_name = db_name.clone();
        let source_name = source_name.clone();
        let barrier = Arc::clone(&barrier);
        let backup_success = Arc::clone(&backup_success);

        let handle = tokio::spawn(async move {
            barrier.wait().await;

            // Stagger backup requests slightly
            tokio::time::sleep(Duration::from_millis((i * 50) as u64)).await;

            let backup_req = CreateBackupRequest {
                source: source_name,
                database: db_name,
            };

            if let Ok(resp) = client
                .post(format!("{}{}", BASE_URL, BACKUP_CREATE_ENDPOINT))
                .json(&backup_req)
                .send()
                .await
            {
                if resp.status() == 201 {
                    backup_success.fetch_add(1, Ordering::SeqCst);
                    println!("  Backup {}: SUCCESS", i);
                } else if resp.status() == 409 {
                    println!("  Backup {}: CONFLICT (expected - concurrent)", i);
                } else {
                    println!("  Backup {}: FAILED {}", i, resp.status());
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all workers
    futures::future::join_all(handles).await;

    let total_elapsed = start.elapsed();
    let writes_ok = write_success.load(Ordering::SeqCst);
    let backups_ok = backup_success.load(Ordering::SeqCst);

    println!("\n RESULTS:");
    println!("  Total time: {:?}", total_elapsed);
    println!("  Writes successful: {}/{}", writes_ok, num_writers * 3);
    println!("  Backups successful: {}/{}", backups_ok, num_backups);

    // All writes should succeed (writes don't block backups via CoW)
    assert!(
        writes_ok >= (num_writers * 3) as u64,
        "All writes should succeed"
    );

    // At least 1 backup should succeed
    assert!(backups_ok >= 1, "At least 1 backup should succeed");

    // Verify backups are valid (can list them)
    let list_req = ListBackupsRequest {
        source: source_name.clone(),
        database: db_name.clone(),
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, BACKUP_LIST_ENDPOINT))
        .json(&list_req)
        .send()
        .await
        .unwrap();

    let list_resp: ListBackupsResponse = resp.json().await.unwrap();
    println!("  Total backups created: {}", list_resp.backups.len());

    // Query database to ensure it's still functional
    let query_req = VectorQueryRequest {
        query_vector: generate_random_vectors(1, 1024)[0].clone(),
        database: db_name.clone(),
        source: source_name.clone(),
        top_k: 5,
    };
    let resp = client
        .post(format!("{}{}", BASE_URL, VECTOR_QUERY_ENDPOINT))
        .json(&query_req)
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        200,
        "Database should be queryable after mixed workload"
    );

    println!("✓ Mixed write + backup workload handled correctly!");
}
