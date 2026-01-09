# Apiarist Check Implementation Status

This document tracks the implementation status of checks in apiarist compared to beekeeper.

## Implementation Status

| Check | Beekeeper | Apiarist | Status |
|-------|-----------|----------|--------|
| pingpong | ✅ | ✅ | Parity |
| peercount | ✅ | ✅ | Semantic difference (see below) |
| fullconnectivity | ✅ | ✅ | Parity |
| smoke | ✅ | ✅ | Parity |
| pushsync | ✅ | ✅ | Parity |
| retrieval | ✅ | ✅ | Parity |
| kademlia | ✅ | ❌ | Removed (see below) |
| settlements | ✅ | ❌ | Pending |
| postage | ✅ | ❌ | Pending |
| pss | ✅ | ❌ | Pending |
| soc | ✅ | ❌ | Pending |
| gsoc | ✅ | ❌ | Pending |
| manifest | ✅ | ❌ | Pending |
| feed | ✅ | ❌ | Pending |
| act | ✅ | ❌ | Pending |
| balances | ✅ | ❌ | Pending |
| cashout | ✅ | ❌ | Pending |
| stake | ✅ | ❌ | Pending |
| withdraw | ✅ | ❌ | Pending |
| gc | ✅ | ❌ | Pending |
| load | ✅ | ❌ | Pending |
| redundancy | ✅ | ❌ | Pending |
| fileretrieval | ✅ | ❌ | Pending |
| datadurability | ✅ | ❌ | Pending |
| longavailability | ✅ | ❌ | Pending |
| networkavailability | ✅ | ❌ | Pending |
| pullsync | ✅ | ❌ | Pending |

## Default Checks

The default checks for Kurtosis testing are:
- `pingpong` - P2P connectivity
- `peercount` - Nodes have connected peers
- `fullconnectivity` - All nodes can reach all other nodes
- `smoke` - Upload/download sanity check
- `pushsync` - Chunk sync to closest nodes
- `retrieval` - Chunk download from different nodes

## Semantic Differences

### peercount

**Beekeeper**: Logs peer counts but never fails.

**Apiarist**: Validates that each node has at least `min_peers` (default: 1). Fails if threshold not met.

### kademlia (Removed)

Not implemented. The check has fundamental design flaws:

1. The `depth` field is actually `storageRadius`, not Kademlia connectivity depth
2. Tests internal state rather than actual routing behavior
3. Redundant with fullconnectivity and pushsync checks

## Check Details

### smoke

Supports both modes:
- **Single-shot** (default): Upload once, verify download
- **Long-running**: Continuous testing with configurable duration

**Options**: `duration_secs`, `file_sizes`, `sync_wait_ms`, `iteration_wait_secs`, `seed`, `batch_id`

### pushsync

Uploads chunks and verifies sync to the closest node via Kademlia routing.

**Options**: `chunks_per_node`, `upload_node_count`, `seed`, `retries`, `retry_delay_ms`

### retrieval

Uploads chunks from one node, downloads from a different node.

**Options**: `chunks_per_node`, `upload_node_count`, `seed`

### fullconnectivity

Node-type-aware peer validation:
- Boot nodes: must have `totalNodes - 1` peers
- Full nodes: must have `fullCapableNodes - 1` peers
- Light nodes: must have at least 1 peer

**Configuration**:
```yaml
cluster:
  bootnode:
    name: bootnode
    api_url: http://bootnode:1633
    node_type: boot  # boot | full | light
  nodes:
    - name: bee-0
      api_url: http://bee-0:1633
      node_type: full
```

## Adding New Checks

1. Review beekeeper implementation in `beekeeper/pkg/check/<name>/`
2. Implement in `src/checks/<name>.rs`
3. Register in `src/checks/registry.rs`
4. Document any semantic differences here
