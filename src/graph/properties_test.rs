use super::*;

#[test]
fn test_insert_and_filter_node() {
    let mut store = PropertyStore::new();
    let node_id = 42;
    store.insert_node(node_id, vec![]).unwrap();
    let filtered = store.filter_nodes(&[node_id]).unwrap();
    assert_eq!(filtered.height(), 1);
    assert_eq!(filtered.column("node_id").unwrap().get(0), node_id.into());
    assert_eq!(filtered.column("is_active").unwrap().get(0), true.into());
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
    assert_eq!(all.column("is_active").unwrap().get(0), false.into());
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
    let node_ids: Vec<_> = filtered.column("node_id").unwrap().iter().map(|v| v.unwrap().try_extract::<NodeId>().unwrap()).collect();
    assert!(node_ids.contains(&1));
    assert!(node_ids.contains(&3));
    assert!(!node_ids.contains(&2));
}
