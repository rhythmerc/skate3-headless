//! The stock gameplay camera graph and runtime. Simulation owns the cadence of
//! CameraRuntime::advance after its complete physical/animation publication; the host draws
//! with its frame (skate-game's `camera.rs`).
mod collision;
mod graph;
mod graph_conditions;
mod graph_subject;
mod publication;
mod runtime;
mod settings;
mod shake_data;
mod shot_data;
mod stock_names;
mod subject;
mod trajectory;
mod world_query;
pub use graph_subject::CameraGraphEnvironment;
pub use publication::{
    CameraAirOutput, CameraAnimationOutput, CameraEventsOutput, CameraGrindOutput,
    CameraOffboardOutput, CameraPreferences, CameraPublicationInputs, CameraStateOutput,
    snapshot as publish_camera_subject,
};
pub use runtime::CameraRuntime;
