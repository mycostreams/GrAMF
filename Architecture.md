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
    - Hashmap with timepoint keys
        - Width (float)
        - Length (float)
        - Active (bool)
        - Properties (dict[str, Value])

There are some stipulations with this. For one, length is an invariant property, so it does not necessarily have to be stored under a time keyframe. In the future we might want to have this as a time-variant property so that we can more accurately represent growing tips. 

The above also assumes that the source and target node are represented in a way to show growth source and growth target. 

Finally, activity can also be an invariant, as we can set the length and width of inactive nodes to 0.0. In the future, we might want to represent septated hyphae as inactive, so I think it is still important to represent it this way. 

There are a whole host of extra features and data that are stored in the stg's, which we might want to put on the edge as a simple dict. One more important property is the hyperedge, which is a single edge connected to many other edges. 

## Snapshot Graph Architecture
The snapshot should not contain any temporal data to make it as simple in memory as possible. 
- Nodes
    - Position
    - ID
- Edges
    - Source node ID
    - Target node ID
    - Width
    - Length
    - Properties (dict[str, Value])

One important snapshot that we can get consistently is to get the graph at a certain timestamp. This is why there is no activation in the snapshot graph. 