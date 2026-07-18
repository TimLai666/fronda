//! `fronda-gen-gateway` binary: read config from the environment, build a stub
//! gateway, and serve Protocol v1.1 over HTTP.

use std::net::SocketAddr;

use fronda_gen_gateway::{app_state, build_router, GatewayConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = GatewayConfig::from_env();

    let addr: SocketAddr = config.bind_addr.parse().map_err(|e| {
        format!(
            "invalid {} '{}': {e}",
            fronda_gen_gateway::config::ADDR_ENV,
            config.bind_addr
        )
    })?;

    config.validate()?;

    // Best-effort: advertise Pollinations' live model list so the picker reflects
    // what the provider actually offers today (the list is volatile). A short
    // timeout + a silent fall-through keep a slow/offline Pollinations from
    // stalling startup — the hardcoded default is used when this returns nothing.
    let poll_base = config
        .pollinations_base
        .clone()
        .unwrap_or_else(|| fronda_gen_gateway::pollinations::DEFAULT_POLLINATIONS_BASE.to_string());
    if let Ok(probe) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        if let Some(models) =
            fronda_gen_gateway::pollinations::fetch_models(&poll_base, &probe).await
        {
            println!("pollinations: advertising live models {models:?}");
            config.pollinations_models = Some(models);
        }
    }

    let has_gemini = config.provider_key("gemini").is_some();
    let state = app_state(config);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "fronda-gen-gateway (Protocol v1.1, stub + pollinations image{}) listening on http://{}",
        if has_gemini { " + gemini image" } else { "" },
        listener.local_addr()?
    );

    axum::serve(listener, build_router(state)).await?;
    Ok(())
}
