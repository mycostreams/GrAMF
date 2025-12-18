use geojson::GeoJson;

use crate::io::stg_graph_io::parse_stg_feature_collection;

enum GraphTypes {
    Stg,
    PlateImage,
}

pub fn load_geojson_from_path<P: AsRef<std::path::Path>>(
    path: P,
) -> Result<GeoJson, Box<dyn std::error::Error>> {
    let data = std::fs::read_to_string(path)?;
    let geojson = data.parse::<GeoJson>()?;
    Ok(geojson)
}

pub fn parse_geojson_to_graphs(geojson: &GeoJson) -> Result<(), Box<dyn std::error::Error>> {
    match geojson {
        GeoJson::FeatureCollection(fc) => parse_feature_collection(fc),
        _ => Err("Expected a FeatureCollection".into()),
    }
}

fn determine_graph_type(
    fc: &geojson::FeatureCollection,
) -> Result<GraphTypes, Box<dyn std::error::Error>> {
    if let Some(fm) = &fc.foreign_members {
        if let Some(graph_type_val) = fm.get("metadata") {
            if graph_type_val.get("timestamps").is_some() {
                return Ok(GraphTypes::Stg);
            } else if graph_type_val.get("timestamp").is_some() {
                return Ok(GraphTypes::PlateImage);
            } else {
                return Err("Invalid graph type".into());
            }
        } else {
            return Err("Missing metadata".into());
        }
    }
    Err("Missing foreign members".into())
}

pub fn parse_feature_collection(
    fc: &geojson::FeatureCollection,
) -> Result<(), Box<dyn std::error::Error>> {
    match determine_graph_type(fc)? {
        GraphTypes::Stg => parse_stg_feature_collection(fc),
        GraphTypes::PlateImage => Err("PlateImage parsing not implemented".into()),
    }
}

pub fn save_geojson_to_path<P: AsRef<std::path::Path>>(
    geojson: &GeoJson,
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = geojson.to_string();
    std::fs::write(path, data)?;
    Ok(())
}

#[test]
fn test_load_geojson_from_path() {
    let geojson = load_geojson_from_path("test_data\\stg.geojson").unwrap();
    assert!(matches!(geojson, GeoJson::FeatureCollection(_)));
    match geojson {
        GeoJson::FeatureCollection(fc) => {
            if let Some(fm) = &fc.foreign_members {
                if let Some(metadata_val) = fm.get("metadata") {
                    // metadata_val is &serde_json::Value
                    println!("metadata: {}", metadata_val);
                }
            }
        }
        _ => panic!("Expected a FeatureCollection"),
    }
}
