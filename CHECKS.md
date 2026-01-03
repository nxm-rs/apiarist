# Apiarist Check Implementation Status

This document tracks the implementation status of checks in apiarist compared to beekeeper, including semantic differences and rationale.

## Implementation Status

| Check | Beekeeper | Apiarist | Status | Notes |
|-------|-----------|----------|--------|-------|
| pingpong | ✅ | ✅ | Implemented | Parity |
| peercount | ✅ | ✅ | Implemented | Semantic difference (see below) |
| fullconnectivity | ✅ | ✅ | Implemented | Parity with node-type awareness (see below) |
| smoke | ✅ | ✅ | Implemented | Single-shot and long-running modes |
| kademlia | ✅ | ❌ | Removed | Fundamentally flawed (see below) |
| pushsync | ✅ | ✅ | Implemented | Parity (see below for batch handling) |
| retrieval | ✅ | ✅ | Implemented | Parity (see below for batch handling) |
| settlements | ✅ | ❌ | Pending | |
| postage | ✅ | ❌ | Pending | |
| pss | ✅ | ❌ | Pending | |
| soc | ✅ | ❌ | Pending | |
| gsoc | ✅ | ❌ | Pending | |
| manifest | ✅ | ❌ | Pending | |
| feed | ✅ | ❌ | Pending | |
| act | ✅ | ❌ | Pending | |
| balances | ✅ | ❌ | Pending | |
| cashout | ✅ | ❌ | Pending | |
| stake | ✅ | ❌ | Pending | |
| withdraw | ✅ | ❌ | Pending | |
| gc | ✅ | ❌ | Pending | |
| load | ✅ | ❌ | Pending | |
| redundancy | ✅ | ❌ | Pending | |
| fileretrieval | ✅ | ❌ | Pending | |
| datadurability | ✅ | ❌ | Pending | |
| longavailability | ✅ | ❌ | Pending | |
| networkavailability | ✅ | ❌ | Pending | |
| pullsync | ✅ | ❌ | Pending | |

## Semantic Differences

### peercount

**Beekeeper behavior**: Purely informational. Logs peer counts but never fails.

```go
// beekeeper just logs, never returns error based on peer count
c.logger.Infof("Node %s. Peers %d/%d. Address: %s", n, len(p), clusterSize-1, overlays[g][n])
return err // always nil
```

**Apiarist behavior**: Validates that each node has at least `min_peers` (default: 1). Fails if threshold not met.

**Rationale**: For interoperability testing, we want to assert that nodes have connected peers, not just log the count. The beekeeper behavior is suitable for monitoring/observability but not for pass/fail testing.

### fullconnectivity

**Beekeeper behavior**:
- Checks internal peer lists via `cluster.Peers()`
- Distinguishes node types (full/light/boot) with different expectations:
  - Full nodes: must have `fullNodeCount - 1` peers
  - Boot nodes: must have `allNodeCount - 1` peers
  - Light nodes: must have at least 1 peer
- Validates that all peers are valid cluster members

**Apiarist behavior**:
- Checks peer lists via `/peers` endpoint
- **Node-type-aware** with same expectations as beekeeper:
  - Boot nodes: must have `totalNodes - 1` peers
  - Full nodes: must have `fullCapableNodes - 1` peers (where fullCapable = boot + full)
  - Light nodes: must have at least 1 peer
- Validates that all peers are valid cluster members
- Reports results with per-type breakdown: "Boot: 1/1, Full: 3/3, Light: 2/2"

**Configuration**:
```yaml
cluster:
  bootnode:
    name: bootnode
    api_url: http://bootnode:1633
    node_type: boot  # boot | full | light (defaults to full)
  nodes:
    - name: bee-full-0
      api_url: http://bee-full-0:1633
      node_type: full
    - name: bee-light-0
      api_url: http://bee-light-0:1633
      node_type: light
```

**Backwards compatibility**: If `node_type` is not specified, defaults to `full`. Existing configs without node types continue to work.

### kademlia (Removed)

**Status**: Removed from apiarist. Not implemented.

**Why it was removed**:

The kademlia check has fundamental design flaws that make it unsuitable for interoperability testing:

1. **Misleading field semantics**: The `depth` field in the topology API response is actually the `storageRadius` (storage depth), not the Kademlia connectivity depth. These are different concepts:
   - Storage radius determines which chunks a node is responsible for storing
   - Kademlia depth determines peer connectivity requirements

2. **Inconsistent values**: With storage incentives enabled:
   - New nodes report `depth=0` (empty reserve, no storage responsibility yet)
   - Bootnodes report `depth=31` (MaxPO, default value)
   - Neither value reflects actual Kademlia connectivity

3. **Tests internal state, not interoperability**: The check validates internal node data structures (bin populations, disconnected peer lists) rather than actual network behavior. A node could pass the kademlia check while being unable to route messages.

4. **Redundant coverage**: The meaningful aspects of Kademlia health are already covered by:
   - `peercount`: Validates nodes have connected peers
   - `fullconnectivity`: Validates all nodes can communicate
   - `pushsync`/`retrieval` (P1): Validates actual Kademlia routing works

5. **Not a true interoperability test**: The check examines a single node's view of its peer table. For interoperability testing, we care about whether nodes from different implementations can actually find and communicate with each other—which is what pingpong and fullconnectivity test.

## P0 Checks (Default)

The default checks for Kurtosis testing are:
- `pingpong` - Basic P2P connectivity
- `peercount` - Nodes have connected peers
- `fullconnectivity` - All nodes can reach all other nodes
- `smoke` - Basic upload/download sanity check

These provide core interoperability validation. Higher-level protocol tests (pushsync, retrieval) should be added as P1 checks.

### smoke

**Beekeeper behavior**: Long-running continuous upload/download test with configurable duration and file sizes.

**Apiarist behavior**: Supports both modes:
- **Single-shot** (default): Upload once, verify download from all nodes
- **Long-running**: Continuous testing with `duration_secs`, multiple `file_sizes`, configurable `sync_wait_ms` and `iteration_wait_secs`

**Options**:
- `duration_secs`: Total run duration (0 = single-shot, default)
- `file_sizes`: Array of sizes to test per iteration (default: [1024])
- `sync_wait_ms`: Wait after upload for network sync (default: 2000)
- `iteration_wait_secs`: Wait between iterations (default: 5)
- `seed`: PRNG seed for reproducibility
- `batch_id`: Specific batch to use (auto-detect if not set)

**Implementation note**: Uploads are performed from the node that owns the postage batch to ensure the batch is available.

### pushsync

**Beekeeper behavior**:
- Uses `cluster.NodeNames()` for upload nodes (ALL nodes including boot, full, light)
- Uses `GetOrCreateMutableBatch()` which creates a batch if needed
- Per-node PRNG seeded from master seed via `PseudoGenerators(seed, n)`
- Random chunk sizes (0-4095 bytes) via `r.Intn(MaxChunkSize)`
- Checks if chunk synced to closest node with retries

**Apiarist behavior**:
- Uses ALL nodes (`ctx.nodes`) for upload nodes (matches beekeeper)
- Per-node PRNG seeded from master seed (matches beekeeper's `PseudoGenerators`)
- Random chunk sizes 0-4095 bytes (matches beekeeper)
- Uses `get_or_create_batch()` per upload node (matches beekeeper's `GetOrCreateMutableBatch`)
- Checks if chunk synced to closest node with retries

**Options**:
- `chunks_per_node`: Chunks to upload per node (default: 1)
- `upload_node_count`: Number of nodes to upload from (default: 1)
- `seed`: PRNG seed for reproducibility
- `retries`: Retry count for sync verification (default: 5)
- `retry_delay_ms`: Delay between retries (default: 1000)

**Batch handling**: Apiarist now implements `get_or_create_batch()` which mirrors beekeeper's `GetOrCreateMutableBatch()`. Each check creates/reuses batches on-demand per upload node, exactly like beekeeper. This follows the architectural principle that apiary handles infrastructure (nodes, network, contracts) while apiarist handles application-level operations (batch creation, uploads, verification).

### retrieval

**Beekeeper behavior**:
- Uses `cluster.FullNodeNames()` for node selection (only full nodes)
- Upload from node[i], download from node[(i+1) % len(nodes)]
- Uses `GetOrCreateMutableBatch()` for batch management
- Per-node PRNG seeded from master seed
- Random chunk sizes 0-4095 bytes
- Compares `chunk.Data()` (span + payload) for verification

**Apiarist behavior**:
- Uses `full_capable_nodes()` for node selection (matches beekeeper's FullNodeNames)
- Same round-robin download node selection as beekeeper
- Per-node PRNG seeded from master seed (matches beekeeper)
- Random chunk sizes 0-4095 bytes (matches beekeeper)
- Compares full chunk data (span + payload) for verification (matches beekeeper)
- Uses `get_or_create_batch()` per upload node (matches beekeeper's `GetOrCreateMutableBatch`)

**Options**:
- `chunks_per_node`: Chunks to upload per node (default: 1)
- `upload_node_count`: Number of nodes to upload from (default: 1)
- `seed`: PRNG seed for reproducibility

**Data format note**: Chunks are uploaded with 8-byte span prefix (little-endian payload length) followed by payload. Downloaded data is compared against this full format (span + payload), matching beekeeper behavior exactly.

## Architectural Separation: Apiary vs Apiarist

The apiary/apiarist architecture follows a clear separation of concerns:

**Apiary (Kurtosis package)** handles infrastructure:
- Ethereum client (Reth)
- Smart contract deployment
- Node key generation
- Node funding (ETH/BZZ)
- Bee node deployment (boot, full, light)
- Network connectivity and peer discovery
- Observability stack (Prometheus, Grafana, Tempo)

**Apiarist (Rust testing tool)** handles application operations:
- Postage batch creation (`get_or_create_batch()`)
- Chunk uploads and downloads
- Data verification
- Protocol testing (pushsync, retrieval)
- Result reporting

This separation means:
1. Apiary configures the **environment** - nodes can start and connect
2. Apiarist performs **application-level testing** - using the network as a user would

Batch creation is explicitly an application operation (paying BZZ to store data), not infrastructure setup. This mirrors beekeeper's design where `GetOrCreateMutableBatch()` is called by checks, not by cluster setup.

## Adding New Checks

When implementing new checks:

1. Review the beekeeper implementation in `beekeeper/pkg/check/<name>/`
2. Determine if the check tests interoperability (hive behavior) or internal state
3. Document any semantic differences in this file
4. Add tests in `src/checks/<name>.rs`
5. Register the check in `src/checks/mod.rs`
