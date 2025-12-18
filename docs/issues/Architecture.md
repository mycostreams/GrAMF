# grAMF Architecture

To get this right, we want to store and manipulate these graphs with two separate realms. There is the context of the User Interface, where we want to show a single graph representation. There is also the context of the total Spatial Temporal Graph (STG). We cannot properly represent all of the temporal properties of the temporal graph within the UI. Therefore, any representation that we are doing of the STG will have to show a subset of the total STG. Therefore, we do not expect to have to represent a total STG in the UI. 

## STG Architecture
We can then represent the components of an STG as such:
- Nodes
    - Position
    - ID
- Edges
    - Source node ID
    - Target node ID
    - Edge cluster ID (see below)
    - Hashmap / list indexed by time indices
        - Width (float)
        - Length (float)
        - Active (bool)
        - Properties (dict[str, Value])

Notes:
- "Edge cluster" replaces "hyperedge". An edge cluster is a continuous path (like a highway in a street graph) and groups edges that act as a single, continuous structure. Use null for an undefined cluster index.
- Time keys are global and stored in graph-level metadata as an ordered list of timestamps. Edge time-variant data should refer to these timestamps by simple integer indices (0, 1, 2, ...). This is a performance-friendly representation and keeps per-edge storage compact.
- We assume linear interpolation between adjacent snapshots (global policy). When querying values at an arbitrary time, linearly interpolate using the two nearest time indices.
- Length is conceptually time-varying (stored per time-index) but may be treated as invariant if not present for all indices.

The above also assumes that the source and target node are represented in a way to show growth source and growth target. 

Finally, activity can also be an invariant, as we can set the length and width of inactive nodes to 0.0. In the future, we might want to represent septated hyphae as inactive, so I think it is still important to represent it this way. 

There are a whole host of extra features and data that are stored in the stg's, which we might want to put on the edge as a simple dict. One more important property is the hyperedge, which is a single edge connected to many other edges. This property is used in a lot of spatial graph analysis pipelines, and is important to get right. We might still want to keep this empty, or at -1. 

## Snapshot Graph Architecture
The snapshot should not contain any temporal keys directly and should be lightweight:
- Nodes
    - Position
    - ID
- Edges
    - Source node ID
    - Target node ID
    - Edge cluster ID (nullable)
    - Width
    - Length
    - Properties (dict[str, Value])

Snapshot types (semantics updated):
- Timestamp Snapshot
  - Represents the graph at a single global timestamp (referenced by its time index).
  - Implementation choice: full graph (all nodes/edges present ever) but properties taken from the selected time index and colored/marked for activity. This enables the full topology to be available and time-variant data to be loaded lazily.
- Growth Snapshot
  - Uses two time indices (t0, t1). Growth for an edge is computed from the raw data as length(t1) - length(t0). Source nodes denote growth start and target nodes denote growth end.
  - Growth may also be represented normalized by interval length if required.
- Full Graph Snapshot
  - Presents full topology (all nodes and edges observed across time); time-variant properties are omitted or set to a "not-loaded" state and can be fetched by time index on demand.

## Additional calculated parameters
We will have to experiment with additionally calculated parameters in the graph, as they can either be stored in the snapshot dict, or they can be stored in the ECS system of the game engine. 

## Regular workflow
We expect to be doing the following workflow: We select a graph edge, and are able to see its parameters. we can edit these as well, maybe just with a button, or a click on the parameter itself.

## Time and storage conventions
- Store an ordered list of global timestamps at the graph metadata level (time_keys: [t0, t1, ...]). Per-edge time data should be arrays aligned to these indices or sparse arrays referencing indices.
- Interpolation: linear between adjacent stored snapshots.
- Lazy-loading: full graph topology is available immediately; time-indexed property arrays may be loaded on demand to save memory and I/O.

## Example minimal STG JSON (recommended pattern)
```json
{
  "id": "stg-001",
  "time_unit": "s",
  "time_keys": [0.0, 1.0, 2.0],
  "nodes": [
    { "id": 1, "pos": [0.0, 1.0, 0.0] }
  ],
  "edges": [
    {
      "id": 10,
      "source": 1,
      "target": 2,
      "edge_cluster": null,
      "time_data": [
        { "length": 0.5, "width": 0.02, "active": true },  // index 0 -> time_keys[0]
        { "length": 1.2, "width": 0.02, "active": true },  // index 1 -> time_keys[1]
        null                                                // index 2 -> not recorded / inactive
      ]
    }
  ],
  "schema_version": "1.0"
}```