//! Key-free proof of the full media pipeline through the *keyless* Pollinations
//! provider: a mock Pollinations server returns fixed JPEG bytes; the gateway's
//! `PollinationsImageProvider` fetches them, stores them, and serves them from
//! `/v1/results/{id}`. Submitting then polling to success yields a result URL that
//! returns the *exact* original JPEG bytes — generation → store → serve → fetch,
//! with no API key anywhere in the loop.

use std::net::SocketAddr;
use std::time::Duration;

use axum::{
    http::header::CONTENT_TYPE as AXUM_CONTENT_TYPE, response::IntoResponse, routing::get, Router,
};
use fronda_gen_gateway::{app_state, build_router, GatewayConfig};
use reqwest::header::CONTENT_TYPE;
use serde_json::{json, Value};

/// Fixed "JPEG" bytes: a real SOI/EOI-framed byte sequence. Byte-equality across
/// the whole pipeline is what proves real bytes flow, not a placeholder scheme.
const FAKE_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
    0xDE, 0xAD, 0xBE, 0xEF, 0xFF, 0xD9,
];

async fn serve_fake_jpeg() -> impl IntoResponse {
    ([(AXUM_CONTENT_TYPE, "image/jpeg")], FAKE_JPEG.to_vec())
}

/// A fake Pollinations endpoint: any `/prompt/...` path (in fact any path) returns
/// the fixed JPEG bytes with `Content-Type: image/jpeg`.
async fn spawn_mock_pollinations() -> SocketAddr {
    let router = Router::new()
        .route("/prompt/{prompt}", get(serve_fake_jpeg))
        .fallback(serve_fake_jpeg);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    addr
}

/// Start a gateway whose Pollinations provider is pointed at `mock_base`. No key
/// is configured — pollinations is always registered. Returns the bound address.
async fn spawn_gateway(mock_base: &str) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = GatewayConfig {
        bind_addr: addr.to_string(),
        auth_token: Some("secret".to_string()),
        public_base: Some(format!("http://{addr}")),
        pollinations_base: Some(mock_base.to_string()),
        ..GatewayConfig::default()
    };

    let router = build_router(app_state(config));
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    addr
}

#[tokio::test]
async fn pollinations_pipeline_serves_exact_original_jpeg_bytes_key_free() {
    let mock_addr = spawn_mock_pollinations().await;
    let gw_addr = spawn_gateway(&format!("http://{mock_addr}")).await;
    let base = format!("http://{gw_addr}");
    let client = reqwest::Client::new();

    // Submit an image job explicitly routed to the pollinations provider — no key.
    let submit: Value = client
        .post(format!("{base}/v1/generate"))
        .bearer_auth("secret")
        .json(&json!({
            "kind": "image",
            "provider": "pollinations",
            "model": "flux",
            "prompt": "a red fox in snow",
            "params": { "width": 256, "height": 256 }
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
            "failed" => panic!("pollinations job failed: {:?}", poll["error"]),
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
    // capability URL. The served bytes must byte-equal the original JPEG.
    let resp = client.get(&result_url).send().await.unwrap();
    assert_eq!(resp.status(), 200, "capability URL served without a token");
    assert_eq!(resp.headers()[CONTENT_TYPE], "image/jpeg");
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), FAKE_JPEG);
}

#[tokio::test]
async fn without_any_key_image_catalog_lists_stub_and_pollinations() {
    // Bare config: no token, no keys. Pollinations is still registered.
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

    let catalog: Value = reqwest::Client::new()
        .get(format!("{base}/v1/providers"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let image = catalog["image"].as_array().unwrap();
    let names: Vec<&str> = image.iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 2, "names were: {names:?}");
    assert!(names.contains(&"stub"), "names were: {names:?}");
    assert!(names.contains(&"pollinations"), "names were: {names:?}");
}
