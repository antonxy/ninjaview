use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum BuildLogEntry {
    #[serde(rename = "build_edge_finished")]
    BuildEdgeFinished {
        edge_id: usize,
        success: bool,
        command: String,
        output: String,
    },
}
