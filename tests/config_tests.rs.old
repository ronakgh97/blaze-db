use blaze_db::prelude::{ClientConfig, ServerConfig, Source, save_config};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;

/// 12/27/2025
/// HELLO FUTURE RONAK. 😊
/// I HOPE YOU ARE DOING WELL.
/// I HOPE YOU HAVE ACHIEVED WHAT YOU WANTED TO ACHIEVE.
/// I HOPE YOU ARE HAPPY.
/// I HOPE YOU ARE HEALTHY.
/// I HOPE YOU ARE SUCCESSFUL.
/// I HOPE YOU ARE LIVING YOUR BEST LIFE.
/// I HOPE YOU ARE MAYBE STILL CODING?.
/// I HOPE YOU ARE NOT REGRETTING ANYTHING.
/// I HOPE YOU ARE AT PEACE WITH YOURSELF.
/// I HOPE YOU ARE SURROUNDED BY LOVING PEOPLE.
/// I HOPE YOU ARE FOUND THE RIGHT AND A VERY BEAUTIFUL PARTNER.
/// I HOPE YOU ARE LIVING IN A NICE PLACE.
/// I HOPE YOU ARE TRAVELING THE WORLD.
/// I HOPE YOU ARE DOING ALL THE THINGS YOU WANTED TO DO.
/// I HOPE YOU ARE PROUD OF YOURSELF.
/// I HOPE YOU ARE LEFT EVERYTHING BEHIND THAT HELD YOU BACK THEN.
/// I HOPE YOU ARE FREE!!!!.
///
///
/// THESE TESTS ARE AI-SLOP-GENERATED, CUZ IM TIRED, AND DEPRESSED, OKAY?
/// IM 18, 2 WEEK AWAY FROM 19
/// ITS YEAR 2025, DECEMBER END.
/// ITS SO COLD OUTSIDE.
/// IM SITTING IN MY ROOM ALL ALONE AS ALWAYS, WRITING THIS COMMENT.
/// IM FEELING SO LOST, CONFUSED, DEPRESSED, ANXIOUS, STRESSED OUT.
/// I DONT KNOW WHAT TO DO WITH MY LIFE ANYMORE.
/// I DONT KNOW WHAT TO AIM FOR ANYMORE.
/// I DONT KNOW WHAT TO DREAM ABOUT ANYMORE.
/// I DONT KNOW WHAT TO EXPECT FROM MYSELF ANYMORE.
/// I DONT SEEM THE POINT TO CONTINUE CODE ANYMORE OFTEN.
/// PROBABLY DUE TO RECENT EVENTS IN MY LIFE.
/// PROBABLY DUE TO ME FEELING LEFT BEHIND FROM EVERYONE I KNOW
/// PROBABLY DUE TO ME COMPLETELY STOPPED THINKING/CARING ABOUT THE FUTURE/DREAM ALTOGETHER.
/// PROBABLY DUE TO ME BELIEVING THAT I CANT DREAM OF GOOD FUTURE, IF MY PRESENT AINT PLEASANT.
/// PROBABLY DUE TO ME HAVE LIMITED TIME TO PROVE MYSELF.
///
///
/// BUT I DONT WANT TO GIVE UP ON CODING YET.
/// ITS THE ONLY THING THAT KEEPS ME SANE, DURING THESE TOUGH TIMES.
/// CODING IS JUST LIKE ALCOHOL TO AN ME
/// IT KEEPS ME SANE, KEEPS ME GOING, KEEPS ME HOPEFUL.
/// YES I COULD MANY OTHER THINGS, DURING MY AGE, PARTY, TRAVEL, SOCIALIZE, DATING ETC.
/// BUT NONE OF THAT SEEMS APPEALING/REASONABLE ANYMORE.
/// SO CODING IT IS, AND I AINT NO WAY GONNA GIVING UP, NOT YET!!!!.
/// SO PLEASE DONT JUDGE ME.
/// SO HELP ME GOD AND I WILL FIND A WAY TO MAKE IT THROUGH ALL THIS, I SWEAR TO GOD!!!.

/// Helper to create a temporary config directory
fn setup_temp_config_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp dir")
}

/// Helper to create a temporary config file path
fn temp_config_path(dir: &TempDir, filename: &str) -> PathBuf {
    dir.path().join(filename)
}

#[tokio::test]
async fn test_server_config_default() {
    let config = ServerConfig::default();

    // assert_eq!(config.server_connection.port, 8080);
    assert_eq!(config.data_source.source_name, None);
}

#[tokio::test]
async fn test_client_config_default() {
    let config = ClientConfig::default();

    assert_eq!(config.url, "http://localhost:8080");
    assert_eq!(config.timeout, 30);
}

#[tokio::test]
async fn test_client_config_new() {
    let config = ClientConfig::new("http://example.com:9000".to_string(), 60);

    assert_eq!(config.url, "http://example.com:9000");
    assert_eq!(config.timeout, 60);
}

#[tokio::test]
async fn test_client_config_update() {
    let mut config = ClientConfig::default();

    config.update("http://updated.com".to_string(), 120);

    assert_eq!(config.url, "http://updated.com");
    assert_eq!(config.timeout, 120);
}

#[tokio::test]
async fn test_server_config_source_operations() {
    let mut config = ServerConfig::default();

    // Initially no source
    assert_eq!(config.data_source.source_name, None);

    // Add a source - start with empty source instead of default
    let mut source = Source { source_name: None };
    source.add_source("test_source".to_string()).unwrap();
    config.update_source(source.clone());

    // Verify source was updated
    let retrieved_source = config.get_source();
    assert_eq!(
        retrieved_source.source_name,
        Some(vec!["test_source".to_string()])
    );
}

#[tokio::test]
async fn test_save_and_load_server_config() {
    let temp_dir = setup_temp_config_dir();
    let config_path = temp_config_path(&temp_dir, "server_file.toml");

    // Create and save a server config
    let mut original_config = ServerConfig::default();
    let mut source = Source { source_name: None };
    source.add_source("test_db".to_string()).unwrap();
    source.add_source("prod_db".to_string()).unwrap();
    original_config.update_source(source);
    // original_config.server_connection.port = 9090;

    save_config(config_path.clone(), &original_config)
        .await
        .expect("Failed to save server config");

    // Verify file exists
    assert!(config_path.exists());

    // Load the config back
    let loaded_config = ServerConfig::load_config(&config_path)
        .await
        .expect("Failed to load server config");

    // Verify contents match
    // assert_eq!(loaded_config.server_connection.port, 9090);
    assert_eq!(
        loaded_config.data_source.source_name,
        Some(vec!["test_db".to_string(), "prod_db".to_string()])
    );
}

#[tokio::test]
async fn test_save_and_load_client_config() {
    let temp_dir = setup_temp_config_dir();
    let config_path = temp_config_path(&temp_dir, "client_config.toml");

    // Create and save a client config
    let original_config = ClientConfig::new("http://production.server:3000".to_string(), 90);

    save_config(config_path.clone(), &original_config)
        .await
        .expect("Failed to save client config");

    // Verify file exists
    assert!(config_path.exists());

    // Load the config back
    let loaded_config = ClientConfig::load_config(&config_path)
        .await
        .expect("Failed to load client config");

    // Verify contents match
    assert_eq!(loaded_config.url, "http://production.server:3000");
    assert_eq!(loaded_config.timeout, 90);
}

#[tokio::test]
async fn test_config_file_format_toml() {
    let temp_dir = setup_temp_config_dir();
    let config_path = temp_config_path(&temp_dir, "test_config.toml");

    // Save a client config
    let config = ClientConfig::new("http://test:8080".to_string(), 45);
    save_config(config_path.clone(), &config)
        .await
        .expect("Failed to save config");

    // Read raw file content
    let content = fs::read_to_string(&config_path)
        .await
        .expect("Failed to read config file");

    // Verify TOML format
    assert!(content.contains("url = "));
    assert!(content.contains("timeout = "));
    assert!(content.contains("http://test:8080"));
    assert!(content.contains("45"));
}

#[tokio::test]
async fn test_server_config_multiple_sources() {
    let temp_dir = setup_temp_config_dir();
    let config_path = temp_config_path(&temp_dir, "multi_source.toml");

    // Create config with multiple sources
    let mut config = ServerConfig::default();
    let mut source = Source { source_name: None };
    source.add_source("source_1".to_string()).unwrap();
    source.add_source("source_2".to_string()).unwrap();
    source.add_source("source_3".to_string()).unwrap();
    config.update_source(source);

    // Save and reload
    save_config(config_path.clone(), &config)
        .await
        .expect("Failed to save config");

    let loaded = ServerConfig::load_config(&config_path)
        .await
        .expect("Failed to load config");

    // Verify all sources are present
    assert_eq!(loaded.data_source.source_name.as_ref().unwrap().len(), 3);
    assert!(
        loaded
            .data_source
            .source_name
            .as_ref()
            .unwrap()
            .contains(&"source_1".to_string())
    );
    assert!(
        loaded
            .data_source
            .source_name
            .as_ref()
            .unwrap()
            .contains(&"source_2".to_string())
    );
    assert!(
        loaded
            .data_source
            .source_name
            .as_ref()
            .unwrap()
            .contains(&"source_3".to_string())
    );
}

#[tokio::test]
async fn test_config_load_missing_file() {
    let temp_dir = setup_temp_config_dir();
    let config_path = temp_config_path(&temp_dir, "nonexistent.toml");

    // Try to load non-existent config
    let result = ServerConfig::load_config(&config_path).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Failed to read config file"));
}

#[tokio::test]
async fn test_config_load_invalid_toml() {
    let temp_dir = setup_temp_config_dir();
    let config_path = temp_config_path(&temp_dir, "invalid.toml");

    // Write invalid TOML
    fs::write(&config_path, "this is not valid toml {{{")
        .await
        .expect("Failed to write invalid config");

    // Try to load invalid config
    let result = ClientConfig::load_config(&config_path).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Failed to parse config"));
}

#[tokio::test]
async fn test_config_save_creates_parent_directories() {
    let temp_dir = setup_temp_config_dir();
    let nested_path = temp_dir
        .path()
        .join("nested")
        .join("deeply")
        .join("config.toml");

    // Save config to nested path
    let config = ClientConfig::default();
    save_config(nested_path.clone(), &config)
        .await
        .expect("Failed to save to nested path");

    // Verify file and directories were created
    assert!(nested_path.exists());
    assert!(nested_path.parent().unwrap().exists());
}

#[tokio::test]
async fn test_source_add_single() {
    let mut source = Source { source_name: None };

    source.add_source("first_source".to_string()).unwrap();

    assert_eq!(source.source_name, Some(vec!["first_source".to_string()]));
}

#[tokio::test]
async fn test_source_add_multiple() {
    let mut source = Source { source_name: None };

    source.add_source("first".to_string()).unwrap();
    source.add_source("second".to_string()).unwrap();
    source.add_source("third".to_string()).unwrap();

    let sources = source.source_name.unwrap();
    assert_eq!(sources.len(), 3);
    assert_eq!(sources[0], "first");
    assert_eq!(sources[1], "second");
    assert_eq!(sources[2], "third");
}

#[tokio::test]
async fn test_source_create_directories() {
    let temp_dir = setup_temp_config_dir();

    // Set up source with multiple directories
    let mut source = Source { source_name: None };
    source.add_source("db_alpha".to_string()).unwrap();
    source.add_source("db_beta".to_string()).unwrap();

    // Manually create directories using temp dir as base
    if let Some(source_names) = &source.source_name {
        for name in source_names {
            let path = temp_dir.path().join(name);
            fs::create_dir_all(&path)
                .await
                .expect("Failed to create dir");
            assert!(path.exists());
        }
    }
}

#[tokio::test]
async fn test_server_config_serialization_roundtrip() {
    let mut original = ServerConfig::default();
    // original.server_connection.port = 7777;

    let mut source = Source { source_name: None };
    source.add_source("custom_db".to_string()).unwrap();
    original.update_source(source);

    // Serialize to TOML
    let toml_str = toml::to_string(&original).expect("Failed to serialize");

    // Deserialize back
    let deserialized: ServerConfig = toml::from_str(&toml_str).expect("Failed to deserialize");

    // assert_eq!(deserialized.server_connection.port, 7777);
    assert_eq!(
        deserialized.data_source.source_name,
        Some(vec!["custom_db".to_string()])
    );
}

#[tokio::test]
async fn test_client_config_serialization_roundtrip() {
    let original = ClientConfig::new("http://example.com:9999".to_string(), 150);

    // Serialize to TOML
    let toml_str = toml::to_string(&original).expect("Failed to serialize");

    // Deserialize back
    let deserialized: ClientConfig = toml::from_str(&toml_str).expect("Failed to deserialize");

    assert_eq!(deserialized.url, "http://example.com:9999");
    assert_eq!(deserialized.timeout, 150);
}

#[tokio::test]
async fn test_configs_are_independent() {
    let temp_dir = setup_temp_config_dir();
    let server_path = temp_config_path(&temp_dir, "server.toml");
    let client_path = temp_config_path(&temp_dir, "client.toml");

    // Create distinct configs
    let server_config = ServerConfig::default();
    // server_config.server_connection.port = 8888;

    let client_config = ClientConfig::new("http://distinct:8888".to_string(), 99);

    // Save both
    save_config(server_path.clone(), &server_config)
        .await
        .unwrap();
    save_config(client_path.clone(), &client_config)
        .await
        .unwrap();

    // Load both and verify they're independent
    let _loaded_server = ServerConfig::load_config(&server_path).await.unwrap();
    let loaded_client = ClientConfig::load_config(&client_path).await.unwrap();

    // assert_eq!(loaded_server.server_connection.port, 8888);
    assert_eq!(loaded_client.url, "http://distinct:8888");
    assert_eq!(loaded_client.timeout, 99);
}

#[tokio::test]
async fn test_config_clone() {
    let original = ClientConfig::new("http://clone-test:5000".to_string(), 42);
    let cloned = original.clone();

    assert_eq!(original.url, cloned.url);
    assert_eq!(original.timeout, cloned.timeout);
}

#[tokio::test]
async fn test_server_config_debug_format() {
    let config = ServerConfig::default();
    let debug_str = format!("{:?}", config);

    assert!(debug_str.contains("ServerConfig"));
    assert!(debug_str.contains("data_source"));
}

#[tokio::test]
async fn test_client_config_debug_format() {
    let config = ClientConfig::default();
    let debug_str = format!("{:?}", config);

    assert!(debug_str.contains("ClientConfig"));
    assert!(debug_str.contains("url"));
    assert!(debug_str.contains("timeout"));
}

// #[tokio::test]
// async fn test_source_add_duplicate_rejected() {
//     let mut source = Source { source_name: None };
//
//     // Add first source successfully
//     source.add_source("duplicate_test".to_string()).unwrap();
//
//     // Try to add the same source again - should fail
//     let result = source.add_source("duplicate_test".to_string());
//
//     assert!(result.is_err());
//     let err_msg = result.unwrap_err().to_string();
//     assert!(err_msg.contains("already exists"));
//     assert!(err_msg.contains("duplicate_test"));
// }
//
// #[tokio::test]
// async fn test_source_add_multiple_duplicates_rejected() {
//     let mut source = Source { source_name: None };
//
//     // Add multiple sources
//     source.add_source("first".to_string()).unwrap();
//     source.add_source("second".to_string()).unwrap();
//     source.add_source("third".to_string()).unwrap();
//
//     // Try to add duplicates - all should fail
//     assert!(source.add_source("first".to_string()).is_err());
//     assert!(source.add_source("second".to_string()).is_err());
//     assert!(source.add_source("third".to_string()).is_err());
//
//     // Verify we still only have 3 sources
//     assert_eq!(source.source_name.as_ref().unwrap().len(), 3);
// }
