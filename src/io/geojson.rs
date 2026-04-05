use crate::graph::controller::GraphEngine;
use crate::graph::topology::{VisualEdge, VisualNode};
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Deserialize, Serialize, Debug)]
pub struct GeoJsonFeatureCollection {
    #[serde(rename = "type")]
    pub feature_type: String,
    pub features: Vec<GeoJsonFeature>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GeoJsonFeature {
    #[serde(rename = "type")]
    pub feature_type: String,
    pub geometry: GeoJsonGeometry,
    pub properties: serde_json::Value,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type")]
pub enum GeoJsonGeometry {
    Point { coordinates: Vec<f64> },
    LineString { coordinates: Vec<Vec<f64>> },
    // Add other geometry types as needed
}

impl GraphEngine {
    pub fn from_geojson_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let geojson: GeoJsonFeatureCollection = serde_json::from_str(&content)?;

        let mut engine = GraphEngine::new();
        let mut node_id_map = HashMap::new();

        // First pass: collect all nodes
        for feature in &geojson.features {
            if let GeoJsonGeometry::Point { coordinates } = &feature.geometry
                && coordinates.len() >= 2
            {
                let x = coordinates[0] as f32;
                let y = coordinates[1] as f32;

                // Scale coordinates to reasonable range (assuming input is in pixels)
                // Scale down by dividing by 1000 to get reasonable world units
                let scale = 0.001;
                let world_pos = Vec2::new(x * scale, y * scale);

                let node_id = feature.properties["id"]
                    .as_u64()
                    .unwrap_or_else(|| feature.properties["id"].as_i64().unwrap_or(0) as u64);

                let visual_node = VisualNode {
                    position: world_pos,
                    color: [0.2, 0.7, 1.0], // default blue color
                    radius: 0.05,
                };

                let graph_node_id = engine.add_node(visual_node, vec![]);
                node_id_map.insert(node_id, graph_node_id);
            }
        }

        // Second pass: collect all edges
        for feature in &geojson.features {
            if let GeoJsonGeometry::LineString { coordinates: _ } = &feature.geometry
                && let (Some(source_id), Some(target_id)) = (
                    feature.properties["source"]
                        .as_u64()
                        .or_else(|| feature.properties["source"].as_i64().map(|v| v as u64)),
                    feature.properties["target"]
                        .as_u64()
                        .or_else(|| feature.properties["target"].as_i64().map(|v| v as u64)),
                )
                && let (Some(&graph_source), Some(&graph_target)) =
                    (node_id_map.get(&source_id), node_id_map.get(&target_id))
            {
                let width = feature.properties["width"].as_f64().unwrap_or(0.02) as f32 * 0.001; // scale width too

                let visual_edge = VisualEdge {
                    color: [1.0, 1.0, 1.0], // default white color
                    width,
                };

                engine.add_edge(graph_source, graph_target, visual_edge);
            }
        }

        Ok(engine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]

    fn test_geojson_parsing() {
        let geojson_str = r#"
        {
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "Point",
                        "coordinates": [100.0, 0.0]
                    },
                    "properties": {
                        "id": 1
                    }
                },
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "Point",
                        "coordinates": [101.0, 1.0]
                    },
                    "properties": {
                        "id": 2
                    }
                },
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [[100.0, 0.0], [101.0, 1.0]]
                    },
                    "properties": {
                        "source": 1,
                        "target": 2,
                        "width": 5.0
                    }
                }
            ]
        }
        "#;

        let geojson: GeoJsonFeatureCollection = serde_json::from_str(geojson_str).unwrap();
        assert_eq!(geojson.feature_type, "FeatureCollection");
        assert_eq!(geojson.features.len(), 3);
    }
}
