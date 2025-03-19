use atrium_api::xrpc::{
    http::{Request, Response},
    types::AuthorizationToken,
    HttpClient, XrpcClient,
};
use atrium_xrpc_client::reqwest::ReqwestClient;

pub struct JwtAuthedClient {
    token: String,
    inner: ReqwestClient,
}

impl JwtAuthedClient {
    pub fn new(base_uri: impl AsRef<str>, token: String) -> Self {
        Self {
            token,
            inner: ReqwestClient::new(base_uri),
        }
    }
}

impl HttpClient for JwtAuthedClient {
    async fn send_http(
        &self,
        request: Request<Vec<u8>>,
    ) -> Result<Response<Vec<u8>>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.inner.send_http(request).await
    }
}

impl XrpcClient for JwtAuthedClient {
    fn base_uri(&self) -> String {
        self.inner.base_uri()
    }

    async fn authorization_token(&self, _: bool) -> Option<AuthorizationToken> {
        Some(AuthorizationToken::Bearer(self.token.clone()))
    }
}
