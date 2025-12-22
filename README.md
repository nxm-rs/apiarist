# apiary

[![CI](https://github.com/nxm-rs/apiary/actions/workflows/ci.yml/badge.svg)](https://github.com/nxm-rs/apiary/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

**Where Swarm nodes go to get stress-tested.**

A testing framework for [Ethereum Swarm](https://ethswarm.org) that actually runs *inside* your test environment instead of awkwardly poking at it from outside. Follows the [Assertoor](https://github.com/ethpandaops/assertoor) pattern because the Ethereum folks figured this out already.

## Quick Start

```bash
# Docker
docker pull ghcr.io/nxm-rs/apiary:latest
docker run --rm ghcr.io/nxm-rs/apiary list

# For the mass-recompilers
cargo install --git https://github.com/nxm-rs/apiary
```

## Usage

```bash
apiary check --config cluster.yaml                    # Unleash all checks
apiary check --config cluster.yaml --checks pingpong  # Just the one, thanks
apiary check --config cluster.yaml --api-port 8080    # With status API
apiary list                                           # What can this thing do?
apiary init > cluster.yaml                            # Generate config
```

## Configuration

```yaml
cluster:
  name: my-cluster
  bootnode:
    name: bootnode
    api_url: http://bootnode:1633
  nodes:
    - name: bee-0
      api_url: http://bee-0:1633
    - name: bee-1
      api_url: http://bee-1:1633

checks:
  pingpong:
    enabled: true
    timeout: 5m
    retries: 3  # Because networks are flaky
```

## Status API

Run with `--api-port` and apiary becomes a well-behaved service:

| Endpoint | What it does |
|----------|--------------|
| `GET /health` | Still alive? |
| `GET /status` | How's it going? (`running` / `completed` / `failed`) |
| `GET /results` | The gory details (202 = still working, 200 = done) |

For Kurtosis users who don't want to grep logs like it's 2005:

```python
plan.wait(
    service_name = "apiary",
    recipe = GetHttpRequestRecipe(port_id = "api", endpoint = "/status"),
    field = "extract.status",
    assertion = "==",
    target_value = "completed"
)
```

## Checks

| Check | What it tests | Status |
|-------|---------------|--------|
| `pingpong` | Can your nodes actually talk to each other? | ✅ |
| `peercount` | Do they have friends? | 🔜 |
| `kademlia` | Is the DHT not completely broken? | 🔜 |

## Why does this exist?

[Beekeeper](https://github.com/ethersphere/beekeeper) exists and it's fine. Really. But:

- **Beekeeper** runs outside your cluster, SSHing in like a sysadmin from 2010
- **Apiary** runs inside your cluster, like a proper containerized service

Also we wanted an excuse to write Rust.

## Etymology

An *apiary* is where you keep bees. We test Bee nodes. The joke writes itself.

## License

[AGPL-3.0-or-later](LICENSE) — because sharing is caring, even when it's legally mandated.
