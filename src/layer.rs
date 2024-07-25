use std::marker::PhantomData;

use tower::Layer;

use crate::{manager::SessionManage, service::SessionManagerService};
/// A layer for providing SessionManagerService.  
/// You can implement session management as a middlware by passing axum::route::Router::layer

#[derive(Debug, Clone)]
pub struct SessionManagerLayer<'a, P, T>
where
    P: SessionManage<T>,
{
    sessions: P,
    session_id_key: &'a str,
    phantome: PhantomData<T>,
}

/// Create SessionManagerLayer.    
impl<'a, P, T> SessionManagerLayer<'a, P, T>
where
P: SessionManage<T>,
{
    /// sessions mean session pool.sessions need to implement SessionManage Trait  
    /// session_id_key is same cookie key you want use.   
    pub fn new(sessions: P, session_id_key: &'a str, phantome: PhantomData<T>) -> Self {
        Self {
            sessions,
            session_id_key,
            phantome,
        }
    }
}

// we can use middleware by using axum::routing::Router::layer
impl<'a, S, P, T> Layer<S> for SessionManagerLayer<'a, P, T>
where
    P: SessionManage<T>,
{
    type Service = SessionManagerService<'a, S, P, T>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionManagerService::new(
            inner,
            self.sessions.clone(),
            self.session_id_key,
            self.phantome.clone(),
        )
    }
}
