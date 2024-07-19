use std::{future::Future, marker::PhantomData, pin::Pin};

use axum_extra::extract::CookieJar;
use http::request::Request;
use tower::Service;

use crate::manager::{SessionManage, UserData, UserState};

#[derive(Debug, Clone)]
pub struct SessionManagerService<'a, S, P, T>
where
    P: SessionManage<T>,
{
    inner: S,
    sessions: P,
    session_id_key: &'a str,
    phantom: PhantomData<T>,
}

impl<'a, S, P, T> SessionManagerService<'a, S, P, T>
where
    P: SessionManage<T>,
{
    pub fn new(inner: S, sessions: P, session_id_key: &'a str, phantom: PhantomData<T>) -> Self {
        Self {
            inner,
            sessions,
            session_id_key,
            phantom,
        }
    }
}

impl<B, S, P, T> Service<Request<B>> for SessionManagerService<'_, S, P, T>
where
    B: Send + 'static,
    S: Service<Request<B>> + Send + Clone + 'static,
    S::Future: Send + 'static,
    P: SessionManage<T> + Send + Clone + 'static,
    T: Clone + Send + Sync + 'static,
    <P as SessionManage<T>>::UserInfo: Clone + Send + Sync + 'static,
    <P as SessionManage<T>>::Error: std::marker::Send,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        // get cookie
        let jar = CookieJar::from_headers(req.headers());
        let cookie_value = jar
            .get(self.session_id_key)
            .map(|cookie| cookie.value().to_owned());

        let clone = self.inner.clone();
        let mut cloned_inner = std::mem::replace(&mut self.inner, clone);
        let cloned_session = self.sessions.clone();

        Box::pin(async move {
            let session_id = match cookie_value {
                Some(session_id) => session_id,
                None => {
                    let val: UserData<<P as SessionManage<T>>::UserInfo> = UserData(UserState::NoCookie);
                    req.extensions_mut()
                        .insert(val);
                    return cloned_inner.call(req).await;
                }
            };

            let query_result = cloned_session.verify_session(&session_id).await;
            match query_result {
                Ok(unchecked_session_data) => match unchecked_session_data {
                    Some(session_data) => {
                        req.extensions_mut()
                            .insert(UserData(UserState::HaveSession(session_data)));
                        return cloned_inner.call(req).await;
                    }
                    None => {
                        let val: UserData<<P as SessionManage<T>>::UserInfo> =
                            UserData(UserState::NoSession);
                        req.extensions_mut().insert(val);
                        return cloned_inner.call(req).await;
                    }
                },
                Err(_e) => {
                    let val: UserData<<P as SessionManage<T>>::UserInfo> =
                        UserData(UserState::NoSession);
                    req.extensions_mut().insert(val);
                    return cloned_inner.call(req).await;
                }
            }
        })
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::HashMap,
        marker::PhantomData,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use axum::{
        body::Body, extract::State, response::IntoResponse, routing::{delete, get, post}, Extension, Json, Router
    };
    use axum_extra::extract::{cookie::Cookie, CookieJar};
    use http::{header::{ACCESS_CONTROL_ALLOW_CREDENTIALS, CONTENT_TYPE, COOKIE, SET_COOKIE}, Request, StatusCode};
    use serde::{Deserialize, Serialize};
    use thiserror::Error;
    use tower::ServiceExt;

    use crate::{SessionManage, SessionManagerLayer, UserData, UserState};

    #[derive(Debug, Clone, Deserialize, Serialize)]
    struct Credential {
        name: String,
        mail: String,
        pass: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct SessionUserData {
        name: String,
        mail: String,
    }

    impl SessionUserData {
        fn new(payload: Credential) -> Self {
            Self {
                name: payload.name,
                mail: payload.mail,
            }
        }
    }

    #[derive(Debug, Clone)]
    struct MockSessionPool {
        pool: Arc<Mutex<HashMap<String, SessionUserData>>>,
    }

    impl MockSessionPool {
        fn new() -> Self {
            Self {
                pool: Arc::default(),
            }
        }
    }

    #[derive(Debug, Clone, Error)]
    enum ServerError {
        #[error("unexpeted error")]
        Unexpect,
    }

    #[async_trait]
    impl SessionManage<Credential> for MockSessionPool {
        type SessionID = String;
        type UserInfo = SessionUserData;
        type Error = ServerError;

        async fn add_session(
            &self,
            session_data: Credential,
        ) -> Result<Self::SessionID, Self::Error> {
            let session_id = uuid::Uuid::new_v4().to_string();
            let session_user_data = SessionUserData::new(session_data);

            self.pool
                .lock()
                .map_err(|_e| ServerError::Unexpect)?
                .insert(session_id.clone(), session_user_data);
            Ok(session_id)
        }

        async fn verify_session(
            &self,
            session_id: &str,
        ) -> Result<Option<Self::UserInfo>, Self::Error> {
            let data = self
                .pool
                .lock()
                .map_err(|_e| ServerError::Unexpect)?
                .get(session_id)
                .map(|e| e.to_owned());
            let data = data.clone();
            Ok(data)
        }

        async fn delete_session(&self, session_id: &str) -> Result<(), Self::Error> {
            let mut text = session_id.to_string();
            for _i in 0..12 {
                text.remove(0);
            }

            self.pool
                .lock()
                .map_err(|_e| ServerError::Unexpect)?
                .remove(&text);

            Ok(())
        }
    }

    fn router() -> Router {
        let sessions = MockSessionPool::new();
        let phantome = PhantomData::default();
        let layer = SessionManagerLayer::new(sessions.clone(), "session-key", phantome);

        Router::new()
            .route("/", get(index))
            .route("/login", post(login))
            .route("/user_data", get(user_data))
            .route("/delete", delete(delete_session))
            .layer(layer)
            .with_state(sessions)
    }

    async fn login(
        jar: CookieJar,
        State(pool): State<MockSessionPool>,
        Json(payload): Json<Credential>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let session_id = pool
            .add_session(payload)
            .await
            .map_err(|_e| StatusCode::INTERNAL_SERVER_ERROR)?;

        let cookie = Cookie::new("session-key", session_id);
        Ok((StatusCode::OK, jar.add(cookie)))
    }

    async fn index() -> impl IntoResponse {
        "Hello"
    }

    async fn user_data(
        Extension(user_data): Extension<UserData<SessionUserData>>,
    ) -> Result<impl IntoResponse, impl IntoResponse> {
        match user_data.0 {
            UserState::HaveSession(data) =>  Ok((StatusCode::OK, Json(data))),
            UserState::NoCookie => Err((StatusCode::UNAUTHORIZED, "no cookie")),
            UserState::NoSession => Err((StatusCode::UNAUTHORIZED, "no session"))
        }
    }

    async fn delete_session(
        jar: CookieJar,
        State(pool): State<MockSessionPool>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let session_id = jar.get("session-key").map(|cookie| cookie.to_owned()).unwrap().to_string();

        pool.delete_session(&session_id).await.map_err(|_e| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(StatusCode::OK)
    }

    #[tokio::test]
    async fn no_cookie() {
        let app = router();
        let req = Request::builder()
            .uri("/user_data")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        
        let byte = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec();
        let body = String::from_utf8(byte).unwrap();
        assert_eq!(body, "no cookie");
    }

    #[tokio::test]
    async fn have_session() {
        let app = router();
        let json_value = Credential {
            name: "test-name".to_string(),
            mail: "test-gmail".to_string(),
            pass: "test-pass".to_string(),
        };

        let req_body = serde_json::to_string(&json_value).unwrap();

        let login_req = Request::builder()
            .uri("/login")
            .method("POST")
            .header(CONTENT_TYPE, "application/json")
            .body(req_body)
            .unwrap();

        let login_res = app.clone().oneshot(login_req).await.unwrap();
        assert_eq!(login_res.status(), StatusCode::OK);

        let verify_req = Request::builder()
        .uri("/user_data")
        .header(ACCESS_CONTROL_ALLOW_CREDENTIALS, "true")
        .header(COOKIE, login_res.headers().get(SET_COOKIE).unwrap())
        .body(Body::empty())
        .unwrap();
        
        let verify_res = app.oneshot(verify_req).await.unwrap();
        assert_eq!(verify_res.status(), StatusCode::OK);

        let byte = axum::body::to_bytes(verify_res.into_body(), usize::MAX).await.unwrap().to_vec();
        let body = String::from_utf8(byte).unwrap();
        let des_body: SessionUserData = serde_json::from_str(&body).unwrap();

        let collect_body = SessionUserData::new(json_value);
        assert_eq!(des_body, collect_body);
    }

    #[tokio::test]
    async fn no_session() {
        let app = router();
        let json_value = Credential {
            name: "test-name".to_string(),
            mail: "test-gmail".to_string(),
            pass: "test-pass".to_string(),
        };
        let req_body = serde_json::to_string(&json_value).unwrap();

        let login_req = Request::builder()
            .uri("/login")
            .method("POST")
            .header(CONTENT_TYPE, "application/json")
            .body(req_body)
            .unwrap();

        let login_res = app.clone().oneshot(login_req).await.unwrap();
        assert_eq!(login_res.status(), StatusCode::OK);

        let delete_req = Request::builder()
        .method("DELETE")
        .header(COOKIE, login_res.headers().get(SET_COOKIE).unwrap())
        .uri("/delete")
        .body(Body::empty())
        .unwrap();

        let delete_res = app.clone().oneshot(delete_req).await.unwrap();
        assert_eq!(delete_res.status(), StatusCode::OK);

        let verify_req = Request::builder()
        .uri("/user_data")
        .header(ACCESS_CONTROL_ALLOW_CREDENTIALS, "true")
        .header(COOKIE, login_res.headers().get(SET_COOKIE).unwrap())
        .body(Body::empty())
        .unwrap();
    
        let verify_res = app.oneshot(verify_req).await.unwrap();
        assert_eq!(verify_res.status(), StatusCode::UNAUTHORIZED);

        let byte = axum::body::to_bytes(verify_res.into_body(), usize::MAX).await.unwrap().to_vec();
        let body = String::from_utf8(byte).unwrap();
        assert_eq!(body, "no session");
    }
}

