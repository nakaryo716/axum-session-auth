//! Session manage system for axum.  
//! ## OverView
//! Sessions are managed using cookies, and session-related operations 
//! (issuing a session ID, checking sessions, deleting sessions) can be implemented with SessionManageTrait, 
//! and session checking is automatically performed at all endpoints by `axum::routing::Router::layer` and [`tower::Service`] as middleware.  
//! At each endpoint, whether a user has a session can be received as UserData via axum::extension::Extention, 
//! and data about the user who has a session is also included there.
//! ## feature
//! The important parts of this crate are the [`SessionManage`] trait, the [`UserData`] struct, and the [`UserState`] enum.  
//! ## usage
//! ### implment session manage system as amiddleware
//! 1. define session pool
//! 2. implement [`SessionManage`] trait for session pool you defined step 1
//! 3. Create [`SessionManagerLayer`] struct by using [`SessionManagerLayer::new()`] method
//! 4. insert [`SessionManagerLayer`] struct you defined step 3, into axum::route::layer 
//! ### get data in your handler
//! 1. use `Extention<UserData<T>>`
//! 2. get data in [`UserData`] struct tuple field
//! 3.  matching [`UserState`] HaveSession, NoCookie, NoSession case  
//! 
//! Please see example for more details.

pub use layer::SessionManagerLayer;
pub use manager::{SessionManage, UserData, UserState};

pub mod layer;
pub mod manager;
mod service;
