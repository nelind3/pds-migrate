use atrium_api::{
    agent::SessionManager,
    types::string::Did,
    xrpc::{
        http::{Request, Response},
        types::AuthorizationToken,
        HttpClient, XrpcClient,
    },
};
use atrium_xrpc_client::reqwest::ReqwestClient;

pub struct JwtSessionManager {
    did: Did,
    token: String,
    inner: ReqwestClient,
}

impl JwtSessionManager {
    pub fn new(did: Did, token: String, base_uri: impl AsRef<str>) -> Self {
        Self {
            did,
            token,
            inner: ReqwestClient::new(base_uri),
        }
    }
}

impl HttpClient for JwtSessionManager {
    async fn send_http(
        &self,
        request: Request<Vec<u8>>,
    ) -> Result<Response<Vec<u8>>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.inner.send_http(request).await
    }
}

impl XrpcClient for JwtSessionManager {
    fn base_uri(&self) -> String {
        self.inner.base_uri()
    }

    async fn authorization_token(&self, _: bool) -> Option<AuthorizationToken> {
        Some(AuthorizationToken::Bearer(self.token.clone()))
    }
}

impl SessionManager for JwtSessionManager {
    async fn did(&self) -> Option<Did> {
        Some(self.did.clone())
    }
}
