use std::marker::PhantomData;

use tower::Layer;

use crate::{manager::SessionManage, service::SessionManagerService};

#[derive(Debug, Clone)]
pub struct SessionManagerLayer<'a, P, T>
where
    P: SessionManage<T>,
{
    sessions: P,
    session_id_key: &'a str,
    phantome: PhantomData<T>,
}

impl<'a, P, T> SessionManagerLayer<'a, P, T>
where
    P: SessionManage<T>,
{
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
