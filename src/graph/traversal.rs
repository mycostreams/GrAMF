use petgraph::visit::Bfs;

use crate::graph::topology::GraphTopology;
use crate::graph::topology::NodeId;

impl GraphTopology {
    pub fn bfs(&self, start: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();
        let start_idx = self.node_id_to_index[&start];

        let mut bfs = Bfs::new(&self.graph, start_idx);

        while let Some(nx) = bfs.next(&self.graph) {
            if let Some(id) = self.node_index_to_id[nx.index()] {
                result.push(id);
            }
        }

        result
    }
}
