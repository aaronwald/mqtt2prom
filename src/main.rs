mod config;
mod metrics;
mod mqtt;
mod parser;
mod server;

use anyhow::Result;
use clap::Parser;
use prometheus_client::registry::Registry;
use std::sync::{Arc, Mutex};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting mqtt2prom - MQTT to Prometheus exporter for Shelly devices");

    // Load configuration
    let config = config::Config::parse();
    info!("Configuration loaded");
    info!("MQTT broker: {}", config.mqtt_server());
    info!("MQTT topic: {}", config.mqtt_topic);
    info!("Metrics port: {}", config.metrics_port);

    // Initialize metrics registry
    let registry = Arc::new(Mutex::new(Registry::default()));
    let metrics = {
        let mut reg = registry.lock().unwrap();
        Arc::new(metrics::ShellyMetrics::new(&mut reg))
    };

    info!("Metrics registry initialized");

    // Spawn HTTP server
    let server_registry = registry.clone();
    let server_port = config.metrics_port;
    tokio::spawn(async move {
        if let Err(e) = server::run(server_port, server_registry).await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    info!("HTTP server started on port {}", config.metrics_port);

    // Run MQTT client (blocks until error or shutdown)
    mqtt::run(config, metrics).await?;

    Ok(())
}
