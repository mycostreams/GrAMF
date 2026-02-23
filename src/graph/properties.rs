use crate::graph::topology::{EdgeId, NodeId};
use polars::prelude::*;

pub struct PropertyStore {
    pub nodes: DataFrame,
    pub edges: DataFrame,
}

impl PropertyStore {
    pub fn new() -> Self {
        let nodes_df = DataFrame::new_infer_height(vec![
            Column::new("node_id".into(), Vec::<NodeId>::new()),
            Column::new("is_active".into(), Vec::<bool>::new()),
        ])
        .unwrap();
        let edges_df = DataFrame::new_infer_height(vec![
            Column::new("edge_id".into(), Vec::<EdgeId>::new()),
            Column::new("is_active".into(), Vec::<bool>::new()),
        ])
        .unwrap();

        Self {
            nodes: nodes_df,
            edges: edges_df,
        }
    }

    pub fn insert_node(&mut self, node_id: NodeId, properties: Vec<Column>) -> PolarsResult<()> {
        let mut row = vec![
            Column::new("node_id".into(), &[node_id]),
            Column::new("is_active".into(), &[true]),
        ];

        row.extend(properties);

        let df = DataFrame::new(1, row)?;
        self.nodes.vstack_mut(&df)?;
        Ok(())
    }

    pub fn mark_inactive(&mut self, node_id: NodeId) -> PolarsResult<()> {
        self.nodes = self
            .nodes
            .clone()
            .lazy()
            .with_column(
                when(col("node_id").eq(lit(node_id)))
                    .then(lit(false))
                    .otherwise(col("is_active"))
                    .alias("is_active"),
            )
            .collect()?;

        Ok(())
    }

    pub fn filter_nodes(&self, ids: &[NodeId]) -> PolarsResult<DataFrame> {
        let id_series = Series::new("filter_ids".into(), ids);

        self.nodes
            .clone()
            .lazy()
            .filter(col("node_id").is_in(lit(id_series), false))
            .filter(col("is_active"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_filter_node() {
        let mut store = PropertyStore::new();
        let node_id = 42;
        store.insert_node(node_id, vec![]).unwrap();
        let filtered = store.filter_nodes(&[node_id]).unwrap();
        assert_eq!(filtered.height(), 1);
        let node_val = filtered
            .column("node_id")
            .unwrap()
            .get(0)
            .unwrap()
            .extract::<u64>()
            .unwrap();
        assert_eq!(node_val, node_id);
        let active_val = filtered
            .column("is_active")
            .unwrap()
            .get(0)
            .unwrap()
            .extract_bool()
            .unwrap();
        assert_eq!(active_val, true);
    }

    #[test]
    fn test_mark_inactive() {
        let mut store = PropertyStore::new();
        let node_id = 99;
        store.insert_node(node_id, vec![]).unwrap();
        store.mark_inactive(node_id).unwrap();
        let filtered = store.filter_nodes(&[node_id]).unwrap();
        assert_eq!(filtered.height(), 0); // Should not return inactive node
        // Check DataFrame directly
        let all = &store.nodes;
        let active_val = all
            .column("is_active")
            .unwrap()
            .get(0)
            .unwrap()
            .extract_bool()
            .unwrap();
        assert_eq!(active_val, false);
    }

    #[test]
    fn test_multiple_nodes_and_filter() {
        let mut store = PropertyStore::new();
        let ids = vec![1, 2, 3];
        for id in &ids {
            store.insert_node(*id, vec![]).unwrap();
        }
        store.mark_inactive(2).unwrap();
        let filtered = store.filter_nodes(&ids).unwrap();
        assert_eq!(filtered.height(), 2);
        let node_ids: Vec<_> = filtered
            .column("node_id")
            .unwrap()
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect();
        assert!(node_ids.contains(&1));
        assert!(node_ids.contains(&3));
        assert!(!node_ids.contains(&2));
    }
}
