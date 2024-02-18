use std::fmt::{self, Display};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StructLogMessage {
    #[serde(rename = "build_edge_started")]
    BuildEdgeStarted(BuildEdgeStarted),

    #[serde(rename = "build_edge_finished")]
    BuildEdgeFinished(BuildEdgeFinished),

    #[serde(rename = "total_edges")]
    TotalEdges { total: usize },

    #[serde(rename = "build_status")]
    BuildStatus { status: BuildStatus },
}

#[derive(Debug, Deserialize)]
pub enum BuildStatus {
    NotStarted,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "finished")]
    Finished,
}

impl Display for BuildStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildStatus::NotStarted => f.write_str("Not Started"),
            BuildStatus::Running => f.write_str("Running"),
            BuildStatus::Finished => f.write_str("Finished"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BuildEdgeStarted {
    pub edge_id: usize,
    pub command: String,
    pub start_time_millis: i64,
}

#[derive(Debug, Deserialize)]
pub struct BuildEdgeFinished {
    pub edge_id: usize,
    pub end_time_millis: i64,
    pub success: bool,
    pub output: String,
}

//Maybe this should be an enum with a running and finished variant, to avoid multiple Options
#[derive(Debug, Deserialize)]
pub struct BuildLogEntry {
    pub edge_id: usize,
    pub success: Option<bool>,
    pub command: String,
    pub output: Option<String>,
    pub start_time_millis: i64,
    pub end_time_millis: Option<i64>,
}

//TODO Should use an ordered hash map, vec is very inefficient
pub struct BuildState {
    pub log_entries: Vec<BuildLogEntry>,
    pub total_edges: usize,
    pub build_status: BuildStatus,
}

impl BuildState {
    pub fn new() -> Self {
        Self {
            log_entries: Vec::new(),
            total_edges: 0,
            build_status: BuildStatus::NotStarted,
        }
    }
    pub fn update(&mut self, message: StructLogMessage) {
        match message {
            StructLogMessage::BuildEdgeStarted(started) => {
                assert!(self
                    .log_entries
                    .iter()
                    .find(|e| e.edge_id == started.edge_id)
                    .is_none());
                self.log_entries.push(BuildLogEntry {
                    edge_id: started.edge_id,
                    success: None,
                    command: started.command,
                    output: None,
                    start_time_millis: started.start_time_millis,
                    end_time_millis: None,
                })
            }
            StructLogMessage::BuildEdgeFinished(finished) => {
                let entry: &mut BuildLogEntry = self
                    .log_entries
                    .iter_mut()
                    .find(|e| e.edge_id == finished.edge_id)
                    .expect("There should be a started entry for every finished entry");

                entry.success = Some(finished.success);
                entry.output = Some(finished.output);
                entry.end_time_millis = Some(finished.end_time_millis);
            }
            StructLogMessage::TotalEdges { total } => self.total_edges = total,
            StructLogMessage::BuildStatus { status } => self.build_status = status,
        }
    }
}
