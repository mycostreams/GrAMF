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
    - Length (float)
    - Hashmap / list indexed by time indices
        - Width (float)
        - State (EdgeState)            # was: Active (bool)
        - Properties (dict[str, Value])

Notes:
- EdgeState (time-varying enum): one of:
  - NotGrownYet
  - Growing (progress: float in [0.0, 1.0])
  - FullyGrown
  - Septating (progress: float in [0.0, 1.0])
  - FullySeptated
  - RegrowingAfterSeptation (progress: float in [0.0, 1.0])

Notes:
- Length is now a time-invariant property on the edge (call it `base_length`: float).
- Real (time-varying) length of an edge is calculated as:
  real_length(t) = base_length * progress(t)
  where progress(t) is derived from the EdgeState at time t (states map to progress in [0,1]; categorical-only states map to 0 or 1).
- Storage: per-time-index entries store state/progress and time-varying fields like width. Do not duplicate `base_length` across time entries. Example JSON shows `base_length` at the edge level.
- Semantics: states with progress expose a numeric progress value in [0,1], used in interpolation and real length calculation. For interpolation between time keys, interpolate numeric progress linearly. Categorical-only states map to progress 0 (NotGrownYet / FullySeptated) or 1 (FullyGrown) for interpolation purposes.
- Activity: edges that are NotGrownYet or FullySeptated are considered inactive; callers may treat real_length==0 as inactive where desired.

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
{
  "id": "stg-001",
  "time_unit": "s",
  "time_keys": [0, 3600, 7200],
  "nodes": [
    { "id": 1, "pos": [0.0, 1.0, 0.0] }
  ],
  "edges": [
    {
      "id": 10,
      "source": 1,
      "target": 2,
      "edge_cluster": null,
      "base_length": 1.2,                             // invariant length in meters (or documented unit)
      "time_data": [
        { "width": 0.0, "state": "NotGrownYet" },            
        { "width": 0.02, "state": "Growing", "progress": 0.25 }, 
        { "width": 0.02, "state": "FullyGrown", "progress": 1.0 }             
      ]
    }
  ],
  "schema_version": "1.0"
}