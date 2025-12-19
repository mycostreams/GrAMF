use bevy::math::Vec2;
use serde::{Deserialize, Serialize};
use crate::graph_model::types::TimeSeries;
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct ParsedGeoJSONStgGraph {
    metadata: ParsedMetadata,
    graph: ParsedGraph,
}
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct ParsedGraph {
    nodes: Vec<Node>,
    edges: Vec<StgEdge>,
}
#[allow(dead_code)]
#[derive(Debug, Default)]
struct Node {
    id: i64,
    position: Vec2,
}
#[allow(dead_code)]
#[derive(Debug, Default)]
struct StgEdge {
    source: i64,
    target: i64,
    length: f32,
    hyperedge: Option<i64>,
    time_series: TimeSeries<EdgeTemporals>,
}
#[allow(dead_code)]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EdgeTemporals {
    width: f32, // Assumed to be diameter, not radius
    // other_properties: serde_json::Value,
}

#[derive(Debug, Default)]
pub struct ParsedMetadata {
    timestamps: Vec<i64>,
    spores: Vec<Spore>,
    timestamp_spores: Vec<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Default)]
struct Spore {
    id: String,
    properties: SporeProperties,
}

#[allow(dead_code)]
#[derive(Debug, Default)]
struct SporeProperties {
    activation: usize,
    position: (f64, f64),
    radius_time_series: TimeSeries<f64>,
}

impl ParsedMetadata {
    pub fn from_stg_feature_collection(
        fc: &geojson::FeatureCollection,
    ) -> Result<ParsedMetadata, Box<dyn std::error::Error>> {
        let metadata = fc
            .foreign_members
            .as_ref()
            .and_then(|fm| fm.get("metadata"))
            .ok_or("Missing metadata")?;

        let metadata = parse_metadata(metadata)?;

        // Placeholder for actual STG graph parsing logic
        println!(
            "Parsing STG FeatureCollection with {} features",
            fc.features.len()
        );
        Ok(metadata)
    }

    pub fn fmt(&self) -> String {
        // Formatter with line breaks
        format!(
            "Metadata \n\ttimestamps: \n{:?}, \n\tspores: \n{:?}, \n\ttimestamp_spores: \n{:?} \n}}",
            self.timestamps, self.spores, self.timestamp_spores
        )
    }
}

fn parse_metadata(
    metadata: &serde_json::Value,
) -> Result<ParsedMetadata, Box<dyn std::error::Error>> {
    let timestamps = metadata
        .get("timestamps")
        .and_then(|ts| ts.as_array())
        .ok_or("Missing timestamps")?
        .iter()
        .map(|t| t.as_i64().ok_or("Invalid timestamp"))
        .collect::<Result<Vec<i64>, _>>()?;

    let spores_value = metadata.get("spores").ok_or("Missing spores")?;

    let spores = parse_spores(spores_value)?;

    let timestamp_spores = metadata
        .get("timestamp_spores")
        .and_then(|ts| ts.as_array())
        .ok_or("Missing timestamp_spores")?
        .iter()
        .map(|t| t.as_i64().ok_or("Invalid timestamp_spore"))
        .collect::<Result<Vec<i64>, _>>()?;

    Ok(ParsedMetadata {
        timestamps,
        spores,
        timestamp_spores,
    })
}

fn parse_spores(
    spores_value: &serde_json::Value,
) -> Result<Vec<Spore>, Box<dyn std::error::Error>> {
    // spores are stored as identifier-keyed objects
    let spores_map = spores_value
        .as_object()
        .ok_or("Spore data is not an object")?;

    let mut spores = Vec::new();

    for (id, spore_val) in spores_map.iter() {
        let properties = parse_spore_properties(spore_val)?;
        spores.push(Spore {
            id: id.to_string(),
            properties,
        });
    }

    Ok(spores)
}

fn parse_spore_properties(
    properties_val: &serde_json::Value,
) -> Result<SporeProperties, Box<dyn std::error::Error>> {
    let activation = properties_val
        .get("activation")
        .and_then(|a| a.as_u64())
        .ok_or("Missing activation")? as usize;

    let position_array = properties_val
        .get("position")
        .and_then(|p| p.as_array())
        .ok_or("Missing position")?;

    if position_array.len() != 2 {
        return Err("Position must have exactly two elements".into());
    }

    let x = position_array[0].as_f64().ok_or("Invalid x position")?;
    let y = position_array[1].as_f64().ok_or("Invalid y position")?;

    let radius_ts_val = properties_val
        .get("radius_time_series")
        .ok_or("Missing radius_time_series")?;

    let radius_time_series = parse_time_series(radius_ts_val)?;

    Ok(SporeProperties {
        activation,
        position: (x, y),
        radius_time_series,
    })
}

fn parse_time_series(
    ts_val: &serde_json::Value,
) -> Result<TimeSeries<f64>, Box<dyn std::error::Error>> {
    let ts_array = ts_val
        .as_object()
        .ok_or("Time series data is not an array")?;

    let mut values = Vec::new();

    for (_, val) in ts_array.iter() {
        let num = val.as_f64().ok_or("Invalid time series value")?;
        values.push(num);
    }

    Ok(TimeSeries::from_vec(values))
}
