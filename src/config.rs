use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// MQTT broker hostname
    #[arg(env = "MQTT_HOST")]
    pub mqtt_host: String,

    /// MQTT broker port
    #[arg(env = "MQTT_PORT", default_value = "1883")]
    pub mqtt_port: u16,

    /// MQTT username
    #[arg(env = "MQTT_USERNAME")]
    pub mqtt_username: String,

    /// MQTT password
    #[arg(env = "MQTT_PASSWORD")]
    pub mqtt_password: String,

    /// MQTT topic to subscribe to
    #[arg(env = "MQTT_TOPIC", default_value = "mostert/shelly/#")]
    pub mqtt_topic: String,

    /// MQTT client ID
    #[arg(env = "MQTT_CLIENT_ID", default_value = "mqtt2prom")]
    pub mqtt_client_id: String,

    /// Prometheus metrics HTTP port
    #[arg(env = "METRICS_PORT", default_value = "8080")]
    pub metrics_port: u16,
}

impl Config {
    pub fn mqtt_server(&self) -> String {
        format!("{}:{}", self.mqtt_host, self.mqtt_port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mqtt_server() {
        let config = Config {
            mqtt_host: "localhost".to_string(),
            mqtt_port: 1883,
            mqtt_username: "user".to_string(),
            mqtt_password: "pass".to_string(),
            mqtt_topic: "test/#".to_string(),
            mqtt_client_id: "test".to_string(),
            metrics_port: 8080,
        };

        assert_eq!(config.mqtt_server(), "localhost:1883");
    }
}
