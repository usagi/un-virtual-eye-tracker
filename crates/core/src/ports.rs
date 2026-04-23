use crate::{
 model::{OutputFrame, TrackingFrame},
 AppResult,
};

pub trait InputReceiver: Send {
 fn source_name(&self) -> &'static str;
 fn poll_frame(&mut self) -> Option<TrackingFrame>;

 fn is_active(&self) -> bool {
  true
 }
}

pub trait OutputBackend: Send {
 fn backend_name(&self) -> &'static str;
 fn apply(&mut self, frame: OutputFrame) -> AppResult<()>;
 fn set_enabled(&mut self, enabled: bool);
 fn is_enabled(&self) -> bool;
}
