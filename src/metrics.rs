use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use std::sync::Arc;

use crate::parser::{extract_device_id, ShellyMessage};

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct DeviceLabels {
    pub device: String,
    pub switch: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct DeviceOnlyLabels {
    pub device: String,
}

pub struct ShellyMetrics {
    power: Family<DeviceLabels, Gauge>,
    voltage: Family<DeviceLabels, Gauge>,
    current: Family<DeviceLabels, Gauge>,
    energy_total: Family<DeviceLabels, Gauge>,
    switch_state: Family<DeviceLabels, Gauge>,
    temperature: Family<DeviceOnlyLabels, Gauge>,
    wifi_rssi: Family<DeviceOnlyLabels, Gauge>,
}

impl ShellyMetrics {
    pub fn new(registry: &mut Registry) -> Self {
        let power = Family::<DeviceLabels, Gauge>::default();
        let voltage = Family::<DeviceLabels, Gauge>::default();
        let current = Family::<DeviceLabels, Gauge>::default();
        let energy_total = Family::<DeviceLabels, Gauge>::default();
        let switch_state = Family::<DeviceLabels, Gauge>::default();
        let temperature = Family::<DeviceOnlyLabels, Gauge>::default();
        let wifi_rssi = Family::<DeviceOnlyLabels, Gauge>::default();

        registry.register(
            "shelly_switch_power_watts",
            "Current power consumption in watts",
            power.clone(),
        );

        registry.register(
            "shelly_switch_voltage_volts",
            "Line voltage in volts",
            voltage.clone(),
        );

        registry.register(
            "shelly_switch_current_amps",
            "Current draw in amps",
            current.clone(),
        );

        registry.register(
            "shelly_switch_energy_total_wh",
            "Total energy consumed in watt-hours",
            energy_total.clone(),
        );

        registry.register(
            "shelly_switch_state",
            "Switch output state (0=off, 1=on)",
            switch_state.clone(),
        );

        registry.register(
            "shelly_temperature_celsius",
            "Device temperature in celsius",
            temperature.clone(),
        );

        registry.register(
            "shelly_wifi_rssi_dbm",
            "WiFi signal strength in dBm",
            wifi_rssi.clone(),
        );

        Self {
            power,
            voltage,
            current,
            energy_total,
            switch_state,
            temperature,
            wifi_rssi,
        }
    }

    pub fn update_from_message(&self, msg: &ShellyMessage) {
        let device_id = extract_device_id(&msg.src);

        if let Some(switch) = &msg.params.switch {
            let switch_id = switch.id.to_string();

            let labels = DeviceLabels {
                device: device_id.clone(),
                switch: switch_id,
            };

            // Update power if present
            if let Some(apower) = switch.apower {
                self.power.get_or_create(&labels).set(apower as i64);
            }

            // Update voltage if present
            if let Some(voltage) = switch.voltage {
                self.voltage
                    .get_or_create(&labels)
                    .set((voltage * 10.0) as i64);
            }

            // Update current if present
            if let Some(current) = switch.current {
                self.current
                    .get_or_create(&labels)
                    .set((current * 1000.0) as i64);
            }

            // Always update energy total
            self.energy_total
                .get_or_create(&labels)
                .set((switch.aenergy.total * 10.0) as i64);

            // Update switch state if present
            if let Some(output) = switch.output {
                self.switch_state
                    .get_or_create(&labels)
                    .set(if output { 1 } else { 0 });
            }

            // Update temperature if present
            if let Some(temp) = &switch.temperature {
                let device_labels = DeviceOnlyLabels {
                    device: device_id.clone(),
                };
                self.temperature
                    .get_or_create(&device_labels)
                    .set((temp.tc * 10.0) as i64);
            }
        }

        // Update WiFi RSSI if present
        if let Some(wifi) = &msg.params.wifi {
            let device_labels = DeviceOnlyLabels {
                device: device_id.clone(),
            };
            self.wifi_rssi
                .get_or_create(&device_labels)
                .set(wifi.rssi as i64);
        }
    }

    pub fn update_power(&self, device: &str, switch: &str, watts: f64) {
        let labels = DeviceLabels {
            device: device.to_string(),
            switch: switch.to_string(),
        };
        self.power.get_or_create(&labels).set(watts as i64);
    }

    pub fn update_voltage(&self, device: &str, switch: &str, volts: f64) {
        let labels = DeviceLabels {
            device: device.to_string(),
            switch: switch.to_string(),
        };
        self.voltage
            .get_or_create(&labels)
            .set((volts * 10.0) as i64);
    }

    pub fn update_current(&self, device: &str, switch: &str, amps: f64) {
        let labels = DeviceLabels {
            device: device.to_string(),
            switch: switch.to_string(),
        };
        self.current
            .get_or_create(&labels)
            .set((amps * 1000.0) as i64);
    }

    pub fn update_energy(&self, device: &str, switch: &str, wh: f64) {
        let labels = DeviceLabels {
            device: device.to_string(),
            switch: switch.to_string(),
        };
        self.energy_total
            .get_or_create(&labels)
            .set((wh * 10.0) as i64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_message;
    use prometheus_client::encoding::text::encode;

    #[test]
    fn test_metrics_registration() {
        let mut registry = Registry::default();
        let _metrics = ShellyMetrics::new(&mut registry);

        let mut buffer = String::new();
        encode(&mut buffer, &registry).unwrap();

        assert!(buffer.contains("shelly_switch_power_watts"));
        assert!(buffer.contains("shelly_switch_voltage_volts"));
        assert!(buffer.contains("shelly_switch_current_amps"));
        assert!(buffer.contains("shelly_switch_energy_total_wh"));
        assert!(buffer.contains("shelly_switch_state"));
        assert!(buffer.contains("shelly_temperature_celsius"));
        assert!(buffer.contains("shelly_wifi_rssi_dbm"));
    }

    #[test]
    fn test_update_individual_metrics() {
        let mut registry = Registry::default();
        let metrics = ShellyMetrics::new(&mut registry);

        metrics.update_power("device1", "0", 125.5);
        metrics.update_voltage("device1", "0", 122.3);
        metrics.update_current("device1", "0", 1.025);
        metrics.update_energy("device1", "0", 3949.949);

        let mut buffer = String::new();
        encode(&mut buffer, &registry).unwrap();

        assert!(buffer.contains("device1"));
        assert!(buffer.contains("switch=\"0\""));
    }

    #[test]
    fn test_update_from_message() {
        let mut registry = Registry::default();
        let metrics = ShellyMetrics::new(&mut registry);

        let json = r#"{
            "src": "shellyplugus-d48afc781ad8",
            "method": "NotifyFullStatus",
            "params": {
                "switch:0": {
                    "id": 0,
                    "output": true,
                    "apower": 125.5,
                    "voltage": 122.3,
                    "current": 1.025,
                    "aenergy": {"total": 3949.949},
                    "temperature": {"tC": 37.9, "tF": 100.1}
                },
                "wifi": {"rssi": -40}
            }
        }"#;

        let msg = parse_message(json).unwrap();
        metrics.update_from_message(&msg);

        let mut buffer = String::new();
        encode(&mut buffer, &registry).unwrap();

        assert!(buffer.contains("d48afc781ad8"));
        assert!(buffer.contains("switch=\"0\""));
    }

    #[test]
    fn test_multiple_devices() {
        let mut registry = Registry::default();
        let metrics = ShellyMetrics::new(&mut registry);

        metrics.update_power("device1", "0", 100.0);
        metrics.update_power("device2", "0", 200.0);

        let mut buffer = String::new();
        encode(&mut buffer, &registry).unwrap();

        assert!(buffer.contains("device1"));
        assert!(buffer.contains("device2"));
    }
}
