use blaze_db::prelude::Provider;

#[test]
fn test_provider_creation() {
    let provider = Provider::init("http://localhost:8080", "test-model");

    assert_eq!(provider.url, "http://localhost:8080");
    assert_eq!(provider.model, "test-model");
}

#[test]
fn test_provider_default_model() {
    let provider = Provider::init("http://localhost:8080", "");

    assert_eq!(provider.model, "text-embedding-nomic-embed-text-v1.5");
}

#[test]
fn test_provider_empty_model_fallback() {
    let provider = Provider::init("http://localhost:8080", "   ");

    // Even whitespace should trigger default model
    assert_eq!(provider.model, "   ");
}

#[test]
fn test_provider_url_preservation() {
    let url = "https://api.example.com:8443/v2/embeddings";
    let model = "custom-model-v1.0";
    let provider = Provider::init(url, model);

    assert_eq!(provider.url, url);
    assert_eq!(provider.model, model);
}

#[test]
fn test_provider_with_various_urls() {
    let test_cases = vec![
        ("http://localhost:1234", "local-model"),
        (
            "https://api.openai.com/v1/embeddings",
            "text-embedding-ada-002",
        ),
        ("http://192.168.1.100:8080/embed", "custom"),
    ];

    for (url, model) in test_cases {
        let provider = Provider::init(url, model);
        assert_eq!(provider.url, url);
        assert_eq!(provider.model, model);
    }
}

#[test]
fn test_provider_clone() {
    let provider1 = Provider::init("http://test.com", "model1");
    let provider2 = provider1.clone();

    assert_eq!(provider1.url, provider2.url);
    assert_eq!(provider1.model, provider2.model);
}

#[test]
fn test_provider_debug() {
    let provider = Provider::init("http://localhost:8080", "test-model");
    let debug_str = format!("{:?}", provider);

    assert!(debug_str.contains("http://localhost:8080"));
    assert!(debug_str.contains("test-model"));
}
