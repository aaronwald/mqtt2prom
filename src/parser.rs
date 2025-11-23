use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Message should be ignored: {0}")]
    IgnoredMessage(String),

    #[error("Missing required field: {0}")]
    #[allow(dead_code)]
    MissingField(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
#[allow(clippy::enum_variant_names)]
pub enum MessageMethod {
    NotifyFullStatus,
    NotifyStatus,
    NotifyEvent,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellyMessage {
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dst: Option<String>,
    pub method: MessageMethod,
    pub params: MessageParams,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageParams {
    #[serde(rename = "switch:0", skip_serializing_if = "Option::is_none")]
    pub switch: Option<SwitchData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wifi: Option<WifiData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sys: Option<SysData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SwitchData {
    pub id: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apower: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voltage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<f64>,
    pub aenergy: EnergyData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<TemperatureData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnergyData {
    pub total: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_minute: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minute_ts: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TemperatureData {
    #[serde(rename = "tC")]
    pub tc: f64,
    #[serde(rename = "tF")]
    pub tf: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WifiData {
    pub rssi: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SysData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<i64>,
}

/// Parse a Shelly MQTT message from JSON
pub fn parse_message(json: &str) -> Result<ShellyMessage, ParserError> {
    let msg: ShellyMessage = serde_json::from_str(json)?;

    // Ignore NotifyEvent messages as per spec
    if msg.method == MessageMethod::NotifyEvent {
        return Err(ParserError::IgnoredMessage("NotifyEvent".to_string()));
    }

    Ok(msg)
}

/// Extract device ID from source field
/// Example: "shellyplugus-d48afc781ad8" -> "d48afc781ad8"
pub fn extract_device_id(src: &str) -> String {
    if let Some(idx) = src.rfind('-') {
        src[idx + 1..].to_string()
    } else {
        src.to_string()
    }
}

/// Check if a message should be processed based on method type
#[allow(dead_code)]
pub fn should_process(method: &MessageMethod) -> bool {
    !matches!(method, MessageMethod::NotifyEvent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_notify_full_status() {
        let json = r#"{
            "src": "shellyplugus-d48afc781ad8",
            "dst": "mostert/shelly/events",
            "method": "NotifyFullStatus",
            "params": {
                "switch:0": {
                    "id": 0,
                    "output": false,
                    "apower": 125.5,
                    "voltage": 122.3,
                    "current": 1.025,
                    "aenergy": {
                        "total": 3949.949
                    },
                    "temperature": {
                        "tC": 37.9,
                        "tF": 100.1
                    }
                },
                "wifi": {
                    "rssi": -40
                }
            }
        }"#;

        let msg = parse_message(json).unwrap();

        assert_eq!(msg.src, "shellyplugus-d48afc781ad8");
        assert_eq!(msg.method, MessageMethod::NotifyFullStatus);

        let switch = msg.params.switch.as_ref().unwrap();
        assert_eq!(switch.id, 0);
        assert_eq!(switch.output, Some(false));
        assert_eq!(switch.apower, Some(125.5));
        assert_eq!(switch.voltage, Some(122.3));
        assert_eq!(switch.current, Some(1.025));
        assert_eq!(switch.aenergy.total, 3949.949);

        let temp = switch.temperature.as_ref().unwrap();
        assert_eq!(temp.tc, 37.9);
        assert_eq!(temp.tf, 100.1);

        let wifi = msg.params.wifi.as_ref().unwrap();
        assert_eq!(wifi.rssi, -40);
    }

    #[test]
    fn test_parse_notify_status_energy_update() {
        let json = r#"{
            "src": "shellyplugus-d48afc781ad8",
            "method": "NotifyStatus",
            "params": {
                "switch:0": {
                    "id": 0,
                    "aenergy": {
                        "by_minute": [0.0, 0.0, 0.0],
                        "minute_ts": 1763918640,
                        "total": 3949.949
                    }
                }
            }
        }"#;

        let msg = parse_message(json).unwrap();

        assert_eq!(msg.method, MessageMethod::NotifyStatus);
        let switch = msg.params.switch.as_ref().unwrap();
        assert_eq!(switch.aenergy.total, 3949.949);
        assert_eq!(switch.aenergy.by_minute, Some(vec![0.0, 0.0, 0.0]));
    }

    #[test]
    fn test_parse_notify_event_ignored() {
        let json = r#"{
            "src": "shellyplugus-d48afc781ad8",
            "method": "NotifyEvent",
            "params": {
                "events": []
            }
        }"#;

        let result = parse_message(json);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParserError::IgnoredMessage(_)
        ));
    }

    #[test]
    fn test_extract_device_id() {
        assert_eq!(
            extract_device_id("shellyplugus-d48afc781ad8"),
            "d48afc781ad8"
        );

        assert_eq!(extract_device_id("shellyht-abc123"), "abc123");

        // Edge case: no dash
        assert_eq!(extract_device_id("nodash"), "nodash");
    }

    #[test]
    fn test_should_process() {
        assert!(should_process(&MessageMethod::NotifyFullStatus));
        assert!(should_process(&MessageMethod::NotifyStatus));
        assert!(!should_process(&MessageMethod::NotifyEvent));
    }

    #[test]
    fn test_invalid_json() {
        let json = r#"{"invalid": "json"}"#;
        let result = parse_message(json);
        assert!(result.is_err());
    }
}
