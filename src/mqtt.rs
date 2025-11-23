use anyhow::{Context, Result};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::metrics::ShellyMetrics;
use crate::parser::{parse_message, MessageMethod};

pub struct MqttHandler {
    client: AsyncClient,
    metrics: Arc<ShellyMetrics>,
}

impl MqttHandler {
    pub fn new(config: &Config, metrics: Arc<ShellyMetrics>) -> Result<(Self, rumqttc::EventLoop)> {
        let mut mqttoptions =
            MqttOptions::new(&config.mqtt_client_id, &config.mqtt_host, config.mqtt_port);

        mqttoptions.set_credentials(&config.mqtt_username, &config.mqtt_password);
        mqttoptions.set_keep_alive(Duration::from_secs(30));
        mqttoptions.set_clean_session(true);

        let (client, eventloop) = AsyncClient::new(mqttoptions, 10);

        Ok((Self { client, metrics }, eventloop))
    }

    pub async fn subscribe(&self, topic: &str) -> Result<()> {
        self.client
            .subscribe(topic, QoS::AtMostOnce)
            .await
            .context("Failed to subscribe to MQTT topic")?;

        info!("Subscribed to topic: {}", topic);
        Ok(())
    }

    pub fn handle_message(&self, topic: &str, payload: &[u8]) {
        // Only process messages from events/rpc topic
        if !topic.ends_with("/events/rpc") {
            debug!("Skipping topic: {}", topic);
            return;
        }

        let payload_str = match std::str::from_utf8(payload) {
            Ok(s) => s,
            Err(e) => {
                warn!("Invalid UTF-8 in payload: {}", e);
                return;
            }
        };

        debug!("Processing message from {}: {}", topic, payload_str);

        match parse_message(payload_str) {
            Ok(msg) => {
                if msg.method == MessageMethod::NotifyEvent {
                    debug!("Ignoring NotifyEvent message");
                    return;
                }

                info!("Processing {:?} from device: {}", msg.method, msg.src);
                self.metrics.update_from_message(&msg);
            }
            Err(e) => {
                warn!("Failed to parse message: {}", e);
            }
        }
    }
}

pub async fn run(config: Config, metrics: Arc<ShellyMetrics>) -> Result<()> {
    loop {
        info!("Connecting to MQTT broker: {}", config.mqtt_server());

        let (handler, mut eventloop) = match MqttHandler::new(&config, metrics.clone()) {
            Ok(h) => h,
            Err(e) => {
                error!("Failed to create MQTT handler: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        if let Err(e) = handler.subscribe(&config.mqtt_topic).await {
            error!("Failed to subscribe: {}", e);
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }

        info!("MQTT connection established");

        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Incoming::Publish(p))) => {
                    handler.handle_message(&p.topic, &p.payload);
                }
                Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                    info!("MQTT connected");
                }
                Ok(Event::Incoming(Incoming::Disconnect)) => {
                    warn!("MQTT disconnected");
                    break;
                }
                Err(e) => {
                    error!("MQTT error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        warn!("MQTT connection lost, reconnecting in 5 seconds...");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_topic_filtering() {
        assert!("mostert/shelly/events/rpc".ends_with("/events/rpc"));
        assert!(!"mostert/shelly/online".ends_with("/events/rpc"));
        assert!(!"other/topic".ends_with("/events/rpc"));
    }
}
