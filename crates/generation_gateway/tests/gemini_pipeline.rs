//! Key-free proof of the full media pipeline: a mock Gemini server returns real
//! PNG bytes as `inlineData`; the gateway's `GeminiImageProvider` decodes them,
//! stores them, and serves them from `/v1/results/{id}`. Submitting then polling
//! to success yields a result URL that returns the *exact* original PNG bytes —
//! generation → store → serve → fetch, no external key.

use std::net::SocketAddr;
use std::time::Duration;

use axum::{Json, Router};
use base64::Engine;
use fronda_gen_gateway::{app_state, build_router, GatewayConfig};
use reqwest::header::CONTENT_TYPE;
use serde_json::{json, Value};

/// A canonical 1x1 transparent PNG, base64-encoded — a real, decodable image.
const PNG_1X1_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

fn original_png() -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(PNG_1X1_B64)
        .unwrap()
}

/// A fake Gemini `generateContent` endpoint: any path returns a fixed response
/// carrying a text part and an `inlineData` image part.
async fn spawn_mock_gemini() -> SocketAddr {
    let response = json!({
        "candidates": [{
            "content": { "parts": [
                { "text": "here is your image" },
                { "inlineData": { "mimeType": "image/png", "data": PNG_1X1_B64 } }
            ]}
        }]
    });
    let router = Router::new().fallback(move || {
        let response = response.clone();
        async move { Json(response) }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    addr
}

/// Start a gateway whose Gemini provider is pointed at `mock_base`, with a
/// configured (dummy) key so it registers. Returns the gateway's bound address.
async fn spawn_gateway(mock_base: &str) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let mut config = GatewayConfig {
        bind_addr: addr.to_string(),
        auth_token: Some("secret".to_string()),
        public_base: Some(format!("http://{addr}")),
        gemini_base: Some(mock_base.to_string()),
        ..GatewayConfig::default()
    };
    // A (dummy) key so the provider registers; the mock ignores it.
    config
        .provider_keys
        .insert("gemini".to_string(), "test-key".to_string());

    let router = build_router(app_state(config));
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    addr
}

#[tokio::test]
async fn gemini_pipeline_serves_exact_original_png_bytes_key_free() {
    let mock_addr = spawn_mock_gemini().await;
    let gw_addr = spawn_gateway(&format!("http://{mock_addr}")).await;
    let base = format!("http://{gw_addr}");
    let client = reqwest::Client::new();

    // Submit an image job explicitly routed to the gemini provider.
    let submit: Value = client
        .post(format!("{base}/v1/generate"))
        .bearer_auth("secret")
        .json(&json!({
            "kind": "image",
            "provider": "gemini",
            "model": "gemini-2.5-flash-image",
            "prompt": "a red logo"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let job_id = submit["jobId"].as_str().unwrap().to_string();
    assert!(!job_id.is_empty());

    // Poll until the background generation task lands.
    let mut result_url = None;
    for _ in 0..200 {
        let poll: Value = client
            .get(format!("{base}/v1/jobs/{job_id}"))
            .bearer_auth("secret")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        match poll["status"].as_str().unwrap() {
            "succeeded" => {
                result_url = Some(poll["resultUrls"][0].as_str().unwrap().to_string());
                break;
            }
            "failed" => panic!("gemini job failed: {:?}", poll["error"]),
            _ => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }
    let result_url = result_url.expect("job did not reach succeeded in time");

    // The result URL points back at the gateway's own /v1/results surface.
    assert!(
        result_url.starts_with(&format!("{base}/v1/results/")),
        "result URL was: {result_url}"
    );

    // Fetch it WITHOUT a bearer token — the result URL is an unauthenticated
    // capability URL, so a generic media downloader (which has no gateway token)
    // can fetch it. The served bytes must byte-equal the original PNG.
    let resp = client.get(&result_url).send().await.unwrap();
    assert_eq!(resp.status(), 200, "capability URL served without a token");
    assert_eq!(resp.headers()[CONTENT_TYPE], "image/png");
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), original_png().as_slice());
}

#[tokio::test]
async fn unknown_result_id_is_404() {
    let mock_addr = spawn_mock_gemini().await;
    let gw_addr = spawn_gateway(&format!("http://{mock_addr}")).await;
    // No bearer: an unauthenticated capability URL returns 404 for an unknown
    // id (not 401), confirming the results route is outside the auth layer.
    let resp = reqwest::Client::new()
        .get(format!("http://{gw_addr}/v1/results/does-not-exist"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn without_key_image_catalog_omits_gemini_and_rejects_it() {
    // No gemini key configured → gateway still starts, gemini not registered. The
    // keyless pollinations provider is always present alongside the stub.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = build_router(app_state(GatewayConfig {
        bind_addr: addr.to_string(),
        ..GatewayConfig::default()
    }));
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    let catalog: Value = client
        .get(format!("{base}/v1/providers"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let image = catalog["image"].as_array().unwrap();
    let names: Vec<&str> = image.iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"stub"), "names were: {names:?}");
    assert!(names.contains(&"pollinations"), "names were: {names:?}");
    assert!(!names.contains(&"gemini"), "names were: {names:?}");

    // Explicitly asking for gemini is an explicit 400, never a silent fallback.
    let resp = client
        .post(format!("{base}/v1/generate"))
        .json(&json!({ "kind": "image", "provider": "gemini", "model": "m", "prompt": "p" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}
