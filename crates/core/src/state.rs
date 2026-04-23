#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingState {
 Active,
 Inactive,
 Lost,
}

impl Default for TrackingState {
 fn default() -> Self {
  Self::Inactive
 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
 Running,
 Paused,
 Stopped,
}

impl Default for PipelineState {
 fn default() -> Self {
  Self::Stopped
 }
}
