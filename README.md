# apiary

**Where Swarm nodes go to get stress-tested.**

A Rust-based testing framework for [Ethereum Swarm](https://ethswarm.org) networks. Runs comprehensive checks against Bee node clusters to verify they're not completely broken.

## Why?

Because [beekeeper](https://github.com/ethersphere/beekeeper) exists but sometimes you want something that:
- Runs as a container service (Assertoor pattern)
- Exposes a status API for orchestration tools
- Is written in Rust

## Usage

```bash
# Run checks against a cluster
apiary check --config cluster.yaml

# Run specific checks
apiary check --config cluster.yaml --checks pingpong,peercount

# With status API for Kurtosis integration
apiary check --config cluster.yaml --api-port 8080 --keep-alive

# List available checks
apiary list

# Generate default config
apiary init > cluster.yaml
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
    retries: 3
```

## Status API

When running with `--api-port`, apiary exposes:

- `GET /health` - Is it alive?
- `GET /status` - Execution progress
- `GET /results` - Check results (202 while running, 200 when done)

Perfect for Kurtosis `plan.wait()` assertions.

## Checks

| Check | Description |
|-------|-------------|
| `pingpong` | Peer connectivity with RTT measurement |

More coming. Eventually.

## Building

```bash
cargo build --release
```

## Docker

```bash
docker build -t apiary .
docker run --rm apiary list
```

## License

AGPL-3.0-or-later
