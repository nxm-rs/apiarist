# apiarist

[![CI](https://github.com/nxm-rs/apiarist/actions/workflows/ci.yml/badge.svg)](https://github.com/nxm-rs/apiarist/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

**The beekeeper who stress-tests your Swarm.**

While everyone else pokes at clusters from outside like nervous beekeepers in hazmat suits, apiarist runs *inside* your enclave. No SSH. No kubectl port-forwards. Just pure, unfiltered node torture.

Follows the [Assertoor](https://github.com/ethpandaops/assertoor) pattern because the Ethereum folks figured this out already.

## Quick Start

```bash
# Docker
docker pull ghcr.io/nxm-rs/apiarist:latest
docker run --rm ghcr.io/nxm-rs/apiarist list

# For the mass-recompilers
cargo install --git https://github.com/nxm-rs/apiarist
```

## Usage

```bash
apiarist check --config cluster.yaml                    # Time to work
apiarist check --config cluster.yaml --checks pingpong  # Just the one, thanks
apiarist check --config cluster.yaml --api-port 8080    # With status API
apiarist list                                           # What can I do?
apiarist init > cluster.yaml                            # Generate config
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

Run with `--api-port` and apiarist becomes a well-behaved service:

| Endpoint | What it does |
|----------|--------------|
| `GET /health` | Still alive? |
| `GET /status` | How's it going? (`running` / `completed` / `failed`) |
| `GET /results` | The gory details (202 = still working, 200 = done) |

For Kurtosis users who don't want to grep logs like it's 2005:

```python
plan.wait(
    service_name = "apiarist",
    recipe = GetHttpRequestRecipe(port_id = "api", endpoint = "/status"),
    field = "extract.status",
    assertion = "==",
    target_value = "completed"
)
```

## Checks

| Check | What it tests | Status |
|-------|---------------|--------|
| `pingpong` | Can your nodes actually talk to each other? | Done |
| `peercount` | Do they have friends? | Soon |
| `kademlia` | Is the DHT not completely broken? | Soon |

## Why not just use Beekeeper?

[Beekeeper](https://github.com/ethersphere/beekeeper) exists and it's fine. Really. But:

- **Beekeeper** runs outside your cluster, SSHing in like a sysadmin from 2010
- **Apiarist** runs inside your cluster, like a proper containerized service

Also we wanted an excuse to write Rust.

## Etymology

An *apiarist* is someone who keeps bees. We stress-test Bee nodes. The beekeeper doesn't just watch - they *work*.

## License

[AGPL-3.0-or-later](LICENSE) - because sharing is caring, even when it's legally mandated.
