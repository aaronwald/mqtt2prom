# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project: mqtt2prom

MQTT to Prometheus exporter for Shelly IoT devices. Rust application deployed to teamwald namespace in varlab K8s cluster.

## Build & Test Commands

```bash
# Build
cargo build
cargo build --release

# Test
cargo test
cargo test -- --nocapture  # Show output

# Lint & Format
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt
cargo fmt --check

# Run locally
export MQTT_HOST=mqtt.varshtat.com
export MQTT_PORT=1883
export MQTT_USERNAME=mqtt2prom
export MQTT_PASSWORD=<password>
export MQTT_TOPIC="mostert/shelly/#"
export METRICS_PORT=8080
export RUST_LOG=info
cargo run
```

## Dependencies

- `rumqttc` - MQTT client (async, auto-reconnect)
- `prometheus-client` - Prometheus metrics exposition
- `axum` - HTTP server for /metrics endpoint
- `tokio` - Async runtime
- `serde`/`serde_json` - JSON parsing for Shelly messages
- `tracing`/`tracing-subscriber` - Structured logging
- `clap` - CLI argument parsing with env var support
- `anyhow`/`thiserror` - Error handling

## Architecture

### Component Overview

```
mqtt2prom/
├── config.rs      # Configuration from environment variables
├── parser.rs      # Shelly JSON message parsing
├── metrics.rs     # Prometheus metrics registry
├── mqtt.rs        # MQTT client with auto-reconnect
├── server.rs      # HTTP server (/metrics, /health)
└── main.rs        # Application entry point
```

### Data Flow

1. **MQTT Subscriber** connects to broker and subscribes to `mostert/shelly/#`
2. **Message Filter** only processes topics ending in `/events/rpc`
3. **Parser** deserializes JSON and validates message type
4. **Metrics Registry** updates Prometheus gauges with device data
5. **HTTP Server** exposes metrics on `/metrics` endpoint
6. **Prometheus** scrapes metrics every 30 seconds

### Message Processing

Shelly devices publish JSON messages in three types:

**NotifyFullStatus** - Complete device state snapshot:
```json
{
  "src": "shellyplugus-d48afc781ad8",
  "method": "NotifyFullStatus",
  "params": {
    "switch:0": {
      "id": 0,
      "output": false,
      "apower": 125.5,
      "voltage": 122.3,
      "current": 1.025,
      "aenergy": {"total": 3949.949},
      "temperature": {"tC": 37.9, "tF": 100.1}
    },
    "wifi": {"rssi": -40}
  }
}
```

**NotifyStatus** - Incremental updates (focus on energy):
```json
{
  "src": "shellyplugus-d48afc781ad8",
  "method": "NotifyStatus",
  "params": {
    "switch:0": {
      "id": 0,
      "aenergy": {
        "total": 3949.949,
        "by_minute": [0.0, 0.0, 0.0]
      }
    }
  }
}
```

**NotifyEvent** - Device events (IGNORED per spec):
```json
{
  "src": "shellyplugus-d48afc781ad8",
  "method": "NotifyEvent",
  "params": {
    "events": [{"component": "sys", "event": "scheduled_restart"}]
  }
}
```

### Parser Implementation

**Key Functions** (`src/parser.rs`):
- `parse_message(json: &str) -> Result<ShellyMessage>` - Main parser
- `extract_device_id(src: &str) -> String` - Extract device ID from "shellyplugus-XXXX"
- `should_process(method: &MessageMethod) -> bool` - Filter NotifyEvent

**Parsing Rules**:
1. Deserialize JSON with serde
2. Check message method (ignore NotifyEvent)
3. Extract device ID from `src` field (e.g., "d48afc781ad8" from "shellyplugus-d48afc781ad8")
4. Parse optional fields (apower, voltage, current, temperature)
5. Always parse `aenergy.total` (required field)

### Metrics Implementation

**Prometheus Metrics** (`src/metrics.rs`):

All metrics use `Gauge` type with labels `{device, switch}`:

| Metric | Unit | Scale | Description |
|--------|------|-------|-------------|
| `shelly_switch_power_watts` | watts | 1:1 | Current power |
| `shelly_switch_voltage_volts` | volts | 10x | Voltage * 10 for precision |
| `shelly_switch_current_amps` | amps | 1000x | Current * 1000 for precision |
| `shelly_switch_energy_total_wh` | wh | 10x | Energy * 10 for precision |
| `shelly_switch_state` | bool | 0/1 | Switch state |
| `shelly_temperature_celsius` | °C | 10x | Temp * 10 for precision |
| `shelly_wifi_rssi_dbm` | dBm | 1:1 | WiFi signal |

**Why scaling?** Prometheus Gauge uses `i64` internally, so we scale floats for precision.

### MQTT Client

**Connection Handling** (`src/mqtt.rs`):
- Auto-reconnect with 5-second backoff on connection loss
- Keep-alive: 30 seconds
- Clean session: true (stateless)
- QoS: AtMostOnce (0) - sufficient for metrics

**Topic Filtering**:
- Subscribe to: `mostert/shelly/#` (all Shelly topics)
- Process only: `*/events/rpc` (filtered in handler)
- Ignore: `*/online`, other topics

### HTTP Server

**Endpoints** (`src/server.rs`):
- `GET /metrics` - Prometheus text format
- `GET /health` - Liveness/readiness probe (returns "OK")

**Implementation**:
- Axum web framework
- Shares metrics registry via Arc<Mutex<Registry>>
- Runs on separate tokio task (non-blocking)

## Testing

### Unit Tests

**Parser Tests** (`src/parser.rs`):
```bash
cargo test parse_notify_full_status
cargo test parse_notify_status_energy_update
cargo test parse_notify_event_ignored
cargo test extract_device_id
```

**Metrics Tests** (`src/metrics.rs`):
```bash
cargo test metrics_registration
cargo test update_individual_metrics
cargo test update_from_message
cargo test multiple_devices
```

**Server Tests** (`src/server.rs`):
```bash
cargo test health_endpoint
cargo test metrics_endpoint
```

### Test Fixtures

Located in `tests/fixtures/`:
- `notify_full_status.json` - Complete device snapshot
- `notify_status.json` - Energy update
- `notify_event.json` - Event message (should be ignored)

### Integration Testing

For local testing with real MQTT broker:

```bash
# 1. Start local Mosquitto (or use mqtt.varshtat.com)
docker run -d -p 1883:1883 eclipse-mosquitto:2.0.20

# 2. Run mqtt2prom
MQTT_HOST=localhost cargo run

# 3. Publish test message
mqttx pub -h localhost -t "mostert/shelly/events/rpc" \
  -m "$(cat tests/fixtures/notify_full_status.json)"

# 4. Check metrics
curl http://localhost:8080/metrics | grep shelly_
```

## Deployment

### Container Build

**Dockerfile** uses multi-stage build:
1. Build stage: `rust:1.75` - compile release binary
2. Runtime stage: `debian:trixie-slim` - minimal runtime
3. Security: runs as non-root user (uid 1000)
4. Size: ~50MB final image

**Build Commands**:
```bash
# On host (not in devcontainer)
docker build -t mqtt2prom:latest .
docker run -p 8080:8080 \
  -e MQTT_HOST=mqtt.varshtat.com \
  -e MQTT_USERNAME=mqtt2prom \
  -e MQTT_PASSWORD=secret \
  mqtt2prom:latest
```

### Kubernetes Deployment

**Location**: `/workspaces/varlab/clusters/homelab/apps/teamwald/mqtt2prom/`

**Files**:
- `deployment.yaml` - Pod spec with env vars
- `service.yaml` - ClusterIP service on port 8080
- `sealed-secret.yaml` - MQTT credentials
- `servicemonitor.yaml` - Prometheus auto-discovery
- `kustomization.yaml` - Kustomize manifest

**Environment**:
- Namespace: `teamwald`
- MQTT Broker: `mosquitto.mosquitto.svc.cluster.local:1883`
- Prometheus: Auto-discovers via ServiceMonitor

**Network Policies** (already configured in varlab):
- Egress: teamwald → mosquitto:1883
- Ingress: observability → teamwald:8080

### Release Process

1. Update `Cargo.toml` version
2. Commit and tag: `git tag v0.1.0 && git push origin v0.1.0`
3. GitHub Actions builds and pushes to `ghcr.io/<user>/mqtt2prom:v0.1.0`
4. Update varlab deployment manifest with new image tag
5. Flux reconciles automatically

## Infrastructure Integration

### MQTT Broker (Mosquitto)
- **Internal**: `mosquitto.mosquitto.svc.cluster.local:1883`
- **External**: `mqtt.varshtat.com:1883`
- **Auth**: Username/password from sealed-secret
- **Topics**: Shelly devices publish to `mostert/shelly/events/rpc`

### Prometheus
- **Internal**: `prometheus-kube-prometheus-prometheus.observability.svc.cluster.local:9090`
- **Discovery**: ServiceMonitor with label `release: kube-prometheus-stack`
- **Scrape**: Every 30 seconds from `/metrics`

### Shelly Device Configuration

Devices configured to publish to Mosquitto:

```bash
# Get current config
curl -X POST -d '{"id":1, "method":"Mqtt.GetConfig"}' http://10.0.3.134/rpc

# Response shows:
# - server: 10.20.0.101:1883 (Mosquitto IP)
# - topic_prefix: mostert/shelly
# - rpc_ntf: true (sends NotifyStatus/NotifyFullStatus)
```

## Related Repositories

- **varlab** - Infrastructure & K8s manifests (`/workspaces/varlab`)
- **magpie** - Similar MQTT consumer pattern (`/workspaces/magpie`)
- **rustwskalshi** - Rust project reference (`/workspaces/rustwskalshi`)

## Constraints

- Stateless design (no local storage)
- Minimal dependencies
- Security: non-root user, read-only filesystem
- Resource limits: ~50m CPU, 64Mi memory (K8s requests)
- Follow Rust idioms and clippy suggestions
- >80% test coverage target

## Common Issues

### "cargo: command not found"
Rust not installed in varlab devcontainer. Build on host machine or add Rust to devcontainer.

### MQTT connection refused
Check MQTT broker is running and credentials are correct:
```bash
kubectl get pods -n mosquitto
kubectl logs -n mosquitto -l app=mosquitto
```

### Metrics not appearing in Prometheus
Check ServiceMonitor and network policies:
```bash
kubectl get servicemonitor -n teamwald mqtt2prom
kubectl describe networkpolicy -n teamwald
```

### Device metrics missing
Check MQTT messages are being published:
```bash
mqttx sub -h mqtt.varshtat.com -u admin -P admin -t "mostert/shelly/#"
```

## Future Enhancements

- Add Shelly H&T support (humidity/temperature sensors)
- Add Shelly Blu Gateway support (Bluetooth devices)
- Create Grafana dashboard
- Add PrometheusRules for alerting
- Add metrics for MQTT connection status
- Add metrics for message processing rate
