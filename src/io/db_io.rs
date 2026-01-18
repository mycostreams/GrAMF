/// Opens (or creates) a filepath at the default application data path.
/// Might need to be moved to IO
pub(crate) fn open_default_path_db() -> String {
    let data_dir = dirs_next::data_dir().unwrap().join("grAMF");
    std::fs::create_dir_all(&data_dir).unwrap();
    let binding = data_dir.join("gramf.db");
    let data_path = binding.to_str().expect("REASON");
    data_path.to_string()
}