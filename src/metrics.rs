use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

use crate::parser::{extract_device_from_topic, extract_device_id, ShellyMessage};

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
    humidity: Family<DeviceOnlyLabels, Gauge>,
    battery_percent: Family<DeviceOnlyLabels, Gauge>,
    battery_voltage: Family<DeviceOnlyLabels, Gauge>,
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
        let humidity = Family::<DeviceOnlyLabels, Gauge>::default();
        let battery_percent = Family::<DeviceOnlyLabels, Gauge>::default();
        let battery_voltage = Family::<DeviceOnlyLabels, Gauge>::default();
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
            "shelly_humidity_percent",
            "Relative humidity percentage",
            humidity.clone(),
        );

        registry.register(
            "shelly_battery_percent",
            "Battery charge percentage",
            battery_percent.clone(),
        );

        registry.register(
            "shelly_battery_voltage",
            "Battery voltage in volts",
            battery_voltage.clone(),
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
            humidity,
            battery_percent,
            battery_voltage,
            wifi_rssi,
        }
    }

    pub fn update_from_message(&self, msg: &ShellyMessage, topic: Option<&str>) {
        // Use topic-derived device name if available, otherwise fall back to MAC
        let device_id = topic
            .and_then(extract_device_from_topic)
            .unwrap_or_else(|| extract_device_id(&msg.src));

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

            // Update energy total if present
            if let Some(aenergy) = &switch.aenergy {
                self.energy_total
                    .get_or_create(&labels)
                    .set((aenergy.total * 10.0) as i64);
            }

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

        // Update temperature from H&T sensor (temperature:0)
        if let Some(temp) = &msg.params.temperature {
            let device_labels = DeviceOnlyLabels {
                device: device_id.clone(),
            };
            self.temperature
                .get_or_create(&device_labels)
                .set((temp.tc * 10.0) as i64);
        }

        // Update humidity from H&T sensor (humidity:0)
        if let Some(humidity) = &msg.params.humidity {
            let device_labels = DeviceOnlyLabels {
                device: device_id.clone(),
            };
            self.humidity
                .get_or_create(&device_labels)
                .set((humidity.rh * 10.0) as i64);
        }

        // Update battery from device power (devicepower:0)
        if let Some(devicepower) = &msg.params.devicepower {
            if let Some(battery) = &devicepower.battery {
                let device_labels = DeviceOnlyLabels {
                    device: device_id.clone(),
                };
                self.battery_percent
                    .get_or_create(&device_labels)
                    .set(battery.percent as i64);
                self.battery_voltage
                    .get_or_create(&device_labels)
                    .set((battery.voltage * 100.0) as i64);
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

    #[allow(dead_code)]
    pub fn update_power(&self, device: &str, switch: &str, watts: f64) {
        let labels = DeviceLabels {
            device: device.to_string(),
            switch: switch.to_string(),
        };
        self.power.get_or_create(&labels).set(watts as i64);
    }

    #[allow(dead_code)]
    pub fn update_voltage(&self, device: &str, switch: &str, volts: f64) {
        let labels = DeviceLabels {
            device: device.to_string(),
            switch: switch.to_string(),
        };
        self.voltage
            .get_or_create(&labels)
            .set((volts * 10.0) as i64);
    }

    #[allow(dead_code)]
    pub fn update_current(&self, device: &str, switch: &str, amps: f64) {
        let labels = DeviceLabels {
            device: device.to_string(),
            switch: switch.to_string(),
        };
        self.current
            .get_or_create(&labels)
            .set((amps * 1000.0) as i64);
    }

    #[allow(dead_code)]
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
        assert!(buffer.contains("shelly_humidity_percent"));
        assert!(buffer.contains("shelly_battery_percent"));
        assert!(buffer.contains("shelly_battery_voltage"));
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
        metrics.update_from_message(&msg, Some("mostert/shelly/plugcoffee/events/rpc"));

        let mut buffer = String::new();
        encode(&mut buffer, &registry).unwrap();

        // Should use topic-derived name "plugcoffee" instead of MAC
        assert!(buffer.contains("plugcoffee"));
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

    #[test]
    fn test_ht_sensor_message() {
        let mut registry = Registry::default();
        let metrics = ShellyMetrics::new(&mut registry);

        let json = r#"{
            "src": "shellyhtg3-3030f9e7d294",
            "dst": "mostert/shelly/temp-main/events",
            "method": "NotifyFullStatus",
            "params": {
                "temperature:0": {"id": 0, "tC": 18.0, "tF": 64.5},
                "humidity:0": {"id": 0, "rh": 38.9},
                "devicepower:0": {
                    "id": 0,
                    "battery": {"V": 5.41, "percent": 70},
                    "external": {"present": false}
                },
                "wifi": {"rssi": -54}
            }
        }"#;

        let msg = parse_message(json).unwrap();
        metrics.update_from_message(&msg, Some("mostert/shelly/temp-main/events/rpc"));

        let mut buffer = String::new();
        encode(&mut buffer, &registry).unwrap();

        // Check temperature (18.0 * 10 = 180)
        assert!(buffer.contains("temp-main"));
        assert!(buffer.contains("shelly_temperature_celsius"));
        // Check humidity (38.9 * 10 = 389)
        assert!(buffer.contains("shelly_humidity_percent"));
        // Check battery
        assert!(buffer.contains("shelly_battery_percent"));
        assert!(buffer.contains("shelly_battery_voltage"));
    }
}
