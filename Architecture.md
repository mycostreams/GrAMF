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
    - Hyperedge ID
    - Hashmap with timepoint keys
        - Width (float)
        - Length (float)
        - Active (bool)
        - Properties (dict[str, Value])

There are some stipulations with this. For one, length is an invariant property, so it does not necessarily have to be stored under a time keyframe. In the future we might want to have this as a time-variant property so that we can more accurately represent growing tips. 

The above also assumes that the source and target node are represented in a way to show growth source and growth target. 

Finally, activity can also be an invariant, as we can set the length and width of inactive nodes to 0.0. In the future, we might want to represent septated hyphae as inactive, so I think it is still important to represent it this way. 

There are a whole host of extra features and data that are stored in the stg's, which we might want to put on the edge as a simple dict. One more important property is the hyperedge, which is a single edge connected to many other edges. This property is used in a lot of spatial graph analysis pipelines, and is important to get right. We might still want to keep this empty, or at -1. 

## Snapshot Graph Architecture
The snapshot should not contain any temporal data to make it as simple in memory as possible. 
- Nodes
    - Position
    - ID
- Edges
    - Source node ID
    - Target node ID
    - Hyperedge ID
    - Width
    - Length
    - Properties (dict[str, Value])

For now, we should focus on three types of snapshot graphs:
### Timestamp Snapshot
This snapshot would EITHER give us only the edges that exist at a certain time point, leading to less memory usage, OR it still gives all of the edges, with a specific color coding (inactive edges a pale gray, the rest colored according to the selected colormap). 

### Growth Snapshot
This snapshot uses two timestamps, and needs to have a 'growth' parameter on all edges. 

### Full Graph snapshot
This shapshot ignores the active bool, and presents us with ALL nodes and edges in the graph.

## Additional calculated parameters
We will have to experiment with additionally calculated parameters in the graph, as they can either be stored in the snapshot dict, or they can be stored in the ECS system of the game engine. 

## Regular workflow
We expect to be doing the following workflow: We select a graph edge, and are able to see its parameters. we can edit these as well, maybe just with a button, or a click on the parameter itself. 