# Apiarist Check Implementation Status

This document tracks the implementation status of checks in apiarist compared to beekeeper, including semantic differences and rationale.

## Implementation Status

| Check | Beekeeper | Apiarist | Status | Notes |
|-------|-----------|----------|--------|-------|
| pingpong | ✅ | ✅ | Implemented | Parity |
| peercount | ✅ | ✅ | Implemented | Semantic difference (see below) |
| fullconnectivity | ✅ | ✅ | Implemented | Parity with node-type awareness (see below) |
| kademlia | ✅ | ❌ | Removed | Fundamentally flawed (see below) |
| pushsync | ✅ | ❌ | Pending | Priority: High |
| retrieval | ✅ | ❌ | Pending | Priority: High |
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
| smoke | ✅ | ❌ | Pending | |
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

These provide core interoperability validation. Higher-level protocol tests (pushsync, retrieval) should be added as P1 checks.

## Adding New Checks

When implementing new checks:

1. Review the beekeeper implementation in `beekeeper/pkg/check/<name>/`
2. Determine if the check tests interoperability (hive behavior) or internal state
3. Document any semantic differences in this file
4. Add tests in `src/checks/<name>.rs`
5. Register the check in `src/checks/mod.rs`
