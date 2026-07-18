//! `fronda-gen-gateway` binary: read config from the environment, build a stub
//! gateway, and serve Protocol v1.1 over HTTP.

use std::net::SocketAddr;

use fronda_gen_gateway::{app_state, build_router, GatewayConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = GatewayConfig::from_env();

    let addr: SocketAddr = config.bind_addr.parse().map_err(|e| {
        format!(
            "invalid {} '{}': {e}",
            fronda_gen_gateway::config::ADDR_ENV,
            config.bind_addr
        )
    })?;

    if !config.is_loopback() && config.auth_token.is_none() {
        eprintln!(
            "warning: bound to a network address ({addr}) without {}; \
             requests are unauthenticated",
            fronda_gen_gateway::config::TOKEN_ENV
        );
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
