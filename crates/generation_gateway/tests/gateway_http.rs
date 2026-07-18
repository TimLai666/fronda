//! Integration tests: spawn the gateway on a random loopback port and drive it
//! with a real HTTP client, proving Protocol v1.1 over the wire.

use std::net::SocketAddr;

use fronda_gen_gateway::{build_router, stub_app_state, GatewayConfig};
use serde_json::{json, Value};

/// Start a gateway on an ephemeral port; returns its bound address.
async fn spawn(token: Option<&str>) -> SocketAddr {
    let config = GatewayConfig {
        bind_addr: "127.0.0.1:0".to_string(),
        auth_token: token.map(|t| t.to_string()),
        ..GatewayConfig::default()
    };
    let router = build_router(stub_app_state(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    addr
}

#[tokio::test]
async fn full_submit_poll_loop_reaches_succeeded_with_result_url() {
    let addr = spawn(Some("secret")).await;
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    // Submit — a v1 client body (no `provider` field).
    let resp = client
        .post(format!("{base}/v1/generate"))
        .bearer_auth("secret")
        .json(&json!({ "kind": "video", "model": "veo-3", "prompt": "a cat surfing" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let submit: Value = resp.json().await.unwrap();
    let job_id = submit["jobId"].as_str().unwrap().to_string();
    assert!(!job_id.is_empty());
    assert_eq!(submit["status"], "queued");

    // Poll #1 → running.
    let first: Value = client
        .get(format!("{base}/v1/jobs/{job_id}"))
        .bearer_auth("secret")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["status"], "running");
    assert!(first.get("resultUrls").is_none());

    // Poll #2 → succeeded with a non-empty result URL.
    let second: Value = client
        .get(format!("{base}/v1/jobs/{job_id}"))
        .bearer_auth("secret")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["status"], "succeeded");
    let urls = second["resultUrls"].as_array().unwrap();
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0], format!("stub://video/{job_id}"));
}

#[tokio::test]
async fn providers_catalog_lists_the_stub_per_kind() {
    let addr = spawn(None).await;
    let catalog: Value = reqwest::Client::new()
        .get(format!("http://{addr}/v1/providers"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    for kind in ["video", "image", "audio"] {
        let entries = catalog[kind].as_array().unwrap();
        assert_eq!(entries.len(), 1, "kind {kind}");
        assert_eq!(entries[0]["name"], "stub");
        assert_eq!(entries[0]["models"][0], format!("stub-{kind}"));
    }
}

#[tokio::test]
async fn missing_token_is_401_and_never_creates_a_job() {
    let addr = spawn(Some("secret")).await;
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/v1/generate"))
        .json(&json!({ "kind": "video", "model": "m", "prompt": "p" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);

    // job-1 would be the first id if any job had been created; it must not exist.
    let poll = client
        .get(format!("{base}/v1/jobs/job-1"))
        .bearer_auth("secret")
        .send()
        .await
        .unwrap();
    assert_eq!(poll.status(), 404);
}

#[tokio::test]
async fn wrong_token_is_401() {
    let addr = spawn(Some("secret")).await;
    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/v1/generate"))
        .bearer_auth("wrong")
        .json(&json!({ "kind": "video", "model": "m", "prompt": "p" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn unknown_provider_is_an_explicit_400() {
    let addr = spawn(None).await;
    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/v1/generate"))
        .json(&json!({ "kind": "video", "model": "m", "prompt": "p", "provider": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    let error = body["error"].as_str().unwrap();
    assert!(error.contains("nope"), "error was: {error}");
    assert!(error.contains("unknown provider"), "error was: {error}");
}
