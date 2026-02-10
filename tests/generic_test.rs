use anyhow::Result;
use blaze_db::prelude::{CreateDatabaseRequest, CreateSourceRequest, InsertRequest, VectorDataDto};
use rand::RngExt;

#[ignore]
#[tokio::test]
async fn a_very_generic_simple_test() -> Result<()> {
    let client = reqwest::Client::new();

    let src_name = format!("test_src-{}", uuid::Uuid::new_v4().to_string());

    let create_source = CreateSourceRequest {
        source_name: src_name.clone(),
    };

    let resp = client
        .post("http://localhost:8080/v1/blazedb/sources/create")
        .json(&create_source)
        .send()
        .await?;

    assert!(
        resp.status().is_success(),
        "Failed to create source: {}",
        resp.status()
    );

    let database_name = format!("test_db-{}", uuid::Uuid::new_v4().to_string());
    let dimensions = 1536;

    let create_vectorbase = CreateDatabaseRequest {
        name: database_name.clone(),
        source: src_name.clone(),
        metrics: None, // default to COSINE
        dimensions,
    };

    let resp = client
        .post("http://localhost:8080/v1/blazedb/databases/create")
        .json(&create_vectorbase)
        .send()
        .await?;

    assert!(
        resp.status().is_success(),
        "Failed to create database: {}",
        resp.status()
    );

    let nodes_num = 5_000;

    let batch_size = 1024;

    let nodes: Vec<VectorDataDto> = generate_random_vectors(nodes_num, dimensions)
        .into_iter()
        .map(|embedding| VectorDataDto {
            embedding,
            metadata: "test_metadata".to_string(),
        })
        .collect();

    // Batch the nodes
    let nodes = nodes
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<Vec<VectorDataDto>>>();

    let insert_resquest = InsertRequest {
        nodes,
        database: database_name.clone(),
        source: src_name.clone(),
    };

    let start_time = std::time::Instant::now();

    let client = reqwest::blocking::Client::new();

    let resp = client
        .post("http://localhost:8080/v1/blazedb/insert")
        .json(&insert_resquest)
        .send()?;

    assert!(
        resp.status().is_success(),
        "Failed to insert vectors: {}",
        resp.status()
    );

    let time_took = start_time.elapsed().as_secs_f64();

    println!("Inserted {} vectors in {} seconds", batch_size, time_took);

    // let query_resquest = QueryRequest {
    //     query: "".to_string(),
    //     database: database_name.clone(),
    //     source: src_name.clone(),
    //     top_k: 5,
    // }

    Ok(())
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
