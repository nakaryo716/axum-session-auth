//! Session manage system for axum.  
//! ## OverView
//! Sessions are managed using cookies, and session-related operations 
//! (issuing a session ID, checking sessions, deleting sessions) can be implemented with SessionManageTrait, 
//! and session checking is automatically performed at all endpoints by axum::routing::Router::layer and tower::Service as middleware.  
//! At each endpoint, whether a user has a session can be received as UserData via axum::extension::Extention, 
//! and data about the user who has a session is also included there.
//! ## feature
//! The important parts of this crate are the SessionManage trait, the UserData struct, and the UserState enum.  

pub use layer::SessionManagerLayer;
pub use manager::{SessionManage, UserData, UserState};

pub mod layer;
pub mod manager;
mod service;
