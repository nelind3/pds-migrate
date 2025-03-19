use atrium_api::{
    agent::{
        atp_agent::{store::MemorySessionStore, AtpAgent},
        Agent,
    },
    app::bsky::actor::{get_preferences, put_preferences},
    com::atproto::{
        server::{create_account, get_service_auth},
        sync::{get_blob, get_repo, list_blobs},
    },
    types::string::{Did, Handle, Nsid},
};
use atrium_common::resolver::Resolver;
use atrium_identity::{
    did::{CommonDidResolver, CommonDidResolverConfig},
    handle::{AtprotoHandleResolver, AtprotoHandleResolverConfig, DnsTxtResolver},
    identity_resolver::{IdentityResolver, IdentityResolverConfig},
};
use atrium_xrpc_client::reqwest::ReqwestClient;
use hickory_resolver::TokioResolver;
use std::{
    io::{self, Write},
    sync::Arc,
};

mod jwt;

struct HickoryDnsTxtResolver {
    resolver: TokioResolver,
}

impl Default for HickoryDnsTxtResolver {
    fn default() -> Self {
        Self {
            resolver: TokioResolver::builder_tokio().unwrap().build(),
        }
    }
}

impl DnsTxtResolver for HickoryDnsTxtResolver {
    async fn resolve(
        &self,
        query: &str,
    ) -> core::result::Result<Vec<String>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(self
            .resolver
            .txt_lookup(query)
            .await?
            .iter()
            .map(|txt| txt.to_string())
            .collect())
    }
}

fn readln(message: Option<impl Into<String>>) -> std::io::Result<Arc<str>> {
    if let Some(message) = message {
        print!("{}", message.into());
        io::stdout().flush()?;
    }
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer)?;
    Ok(buffer.trim().into())
}

#[tokio::main]
async fn main() {
    println!("Please log in to your current PDS. Authenticated access is needed throughout the migration process");
    let identifier = match readln(Some("Identifier (handle or did): ")) {
        Ok(string) => string,
        Err(err) => {
            println!("Could not read username due to error: {err}");
            return;
        }
    };
    let password = match readln(Some("Password: ")) {
        Ok(string) => string.trim().to_string(),
        Err(err) => {
            println!("Could not read password due to error: {err}");
            return;
        }
    };

    let identity_resolver = IdentityResolver::new(IdentityResolverConfig {
        did_resolver: CommonDidResolver::new(CommonDidResolverConfig {
            plc_directory_url: String::from("https://plc.directory"),
            http_client: ReqwestClient::new("").into(),
        }),
        handle_resolver: AtprotoHandleResolver::new(AtprotoHandleResolverConfig {
            dns_txt_resolver: HickoryDnsTxtResolver::default(),
            http_client: ReqwestClient::new("").into(),
        }),
    });
    let identity = match identity_resolver.resolve(identifier.as_ref()).await {
        Ok(identity) => identity,
        Err(err) => {
            println!("Could not resolve identity from identifier {identifier} due to error: {err}");
            return;
        }
    };

    let current_agent = AtpAgent::new(
        ReqwestClient::new(&identity.pds),
        MemorySessionStore::default(),
    );
    if let Err(err) = current_agent.login(identifier, password).await {
        println!(
            "Failed to log in to your account on your current PDS at {} due to error: {err}",
            &identity.pds
        );
        return;
    };
    println!("Log in at {} was successful!", &identity.pds);
    println!();

    // Create new account
    let new_pds_url = match readln(Some(
        "Please type in the URL of the PDS you want to migrate to: ",
    )) {
        Ok(string) => string,
        Err(err) => {
            println!("Could not read the URL of your new PDS due to error: {err}");
            return;
        }
    };
    println!("Creating an account on your new PDS ...");
    let new_agent = AtpAgent::new(
        ReqwestClient::new(&new_pds_url),
        MemorySessionStore::default(),
    );
    println!("Now the details you want for your new account");
    let email = match readln(Some("Email address: ")) {
        Ok(string) => string,
        Err(err) => {
            println!("Could not read your email due to error: {err}");
            return;
        }
    };
    let handle = match Handle::new(
        match readln(Some("Handle: ")) {
            Ok(string) => string,
            Err(err) => {
                println!("Could not read your handle due to error: {err}");
                return;
            }
        }
        .to_string(),
    ) {
        Ok(handle) => handle,
        Err(err) => {
            println!("Handle wasn't accepted because: {err}");
            return;
        }
    };
    let password = match readln(Some(
        "Please type in the password you want to use on your new PDS",
    )) {
        Ok(string) => string,
        Err(err) => {
            println!("Could not read your password due to error: {err}");
            return;
        }
    };
    let invite_code = match readln(Some(
        "Invite code (leave empty if your new PDS doesn't require one): ",
    )) {
        Ok(string) => {
            if string.is_empty() {
                None
            } else {
                Some(string.to_string())
            }
        }
        Err(err) => {
            println!("Could not read your invite code due to error: {err}");
            return;
        }
    };

    let describe_res = match new_agent.api.com.atproto.server.describe_server().await {
        Ok(response) => response,
        Err(err) => {
            println!("com.atproto.server.describeServer at new PDS failed due to error: {err}");
            return;
        }
    };
    let new_pds_did = &describe_res.did;
    let service_jwt_res = match current_agent
        .api
        .com
        .atproto
        .server
        .get_service_auth(
            get_service_auth::ParametersData {
                aud: new_pds_did.clone(),
                lxm: Some(Nsid::new(create_account::NSID.to_string()).unwrap()),
                exp: None,
            }
            .into(),
        )
        .await
    {
        Ok(response) => response,
        Err(err) => {
            println!("com.atproto.server.getServiceAuth at current PDS failed due to error: {err}");
            return;
        }
    };

    let new_jwt_agent = Agent::new(jwt::JwtSessionManager::new(
        Did::new(identity.did.clone()).unwrap(),
        service_jwt_res.token.clone(),
        &new_pds_url,
    ));
    match new_jwt_agent
        .api
        .com
        .atproto
        .server
        .create_account(
            create_account::InputData {
                did: current_agent.did().await,
                email: Some(email.to_string()),
                handle: handle.clone(),
                invite_code,
                password: Some(password.to_string()),
                plc_op: None,
                recovery_key: None,
                verification_code: None,
                verification_phone: None,
            }
            .into(),
        )
        .await
    {
        Ok(_) => (),
        Err(err) => {
            println!("com.atproto.server.createAccount at new PDS failed due to error: {err}");
            return;
        }
    }
    if let Err(err) = new_agent.login(handle.clone(), password).await {
        println!("Failed to log in to your account on your new PDS due to error: {err}");
        return;
    };
    println!("Successfully created account on your new PDS!");
    println!();

    // Migrate data
    println!("Migrating your data");

    let car = match current_agent
        .api
        .com
        .atproto
        .sync
        .get_repo(
            get_repo::ParametersData {
                did: current_agent.did().await.unwrap(),
                since: None,
            }
            .into(),
        )
        .await
    {
        Ok(response) => response,
        Err(err) => {
            println!("com.atproto.sync.getRepo at current PDS failed due to error: {err}");
            return;
        }
    };
    println!("Repository downloaded from old PDS. Importing to new PDS.");

    match new_agent.api.com.atproto.repo.import_repo(car).await {
        Ok(_) => (),
        Err(err) => {
            println!("com.atproto.repo.importRepo at new PDS failed due to error: {err}");
            return;
        }
    }
    println!("Repository successfully migrated");

    let mut listed_blobs = match current_agent
        .api
        .com
        .atproto
        .sync
        .list_blobs(
            list_blobs::ParametersData {
                cursor: None,
                did: current_agent.did().await.unwrap(),
                limit: None,
                since: None,
            }
            .into(),
        )
        .await
    {
        Ok(response) => response,
        Err(err) => {
            println!("com.atproto.sync.listBlobs at old PDS failed due to error: {err}");
            return;
        }
    };

    for cid in listed_blobs.cids.iter() {
        let blob = match current_agent
            .api
            .com
            .atproto
            .sync
            .get_blob(
                get_blob::ParametersData {
                    cid: cid.to_owned(),
                    did: current_agent.did().await.unwrap(),
                }
                .into(),
            )
            .await
        {
            Ok(response) => response,
            Err(err) => {
                println!("com.atproto.sync.getBlob at current PDS failed due to error: {err}");
                return;
            }
        };

        match new_agent.api.com.atproto.repo.upload_blob(blob).await {
            Ok(_) => {
                println!("Blob with CID {:?} migrated", cid)
            },
            Err(err) => {
                println!("com.atproto.repo.uploadBlob at new PDS failed due to error: {err}");
                return;
            }
        };
    }

    let mut cursor = listed_blobs.cursor.clone();
    while cursor.is_some() {
        listed_blobs = match current_agent
            .api
            .com
            .atproto
            .sync
            .list_blobs(
                list_blobs::ParametersData {
                    cursor: cursor.clone(),
                    did: current_agent.did().await.unwrap(),
                    limit: None,
                    since: None,
                }
                .into(),
            )
            .await
        {
            Ok(response) => response,
            Err(err) => {
                println!("com.atproto.sync.listBlobs at old PDS failed due to error: {err}");
                return;
            }
        };

        for cid in listed_blobs.cids.iter() {
            let blob = match current_agent
                .api
                .com
                .atproto
                .sync
                .get_blob(
                    get_blob::ParametersData {
                        cid: cid.to_owned(),
                        did: current_agent.did().await.unwrap(),
                    }
                    .into(),
                )
                .await
            {
                Ok(response) => response,
                Err(err) => {
                    println!("com.atproto.sync.getBlob at current PDS failed due to error: {err}");
                    return;
                }
            };

            match new_agent.api.com.atproto.repo.upload_blob(blob).await {
                Ok(_) => {
                    println!("Blob with CID {:?} migrated", cid)
                },
                Err(err) => {
                    println!("com.atproto.repo.uploadBlob at new PDS failed due to error: {err}");
                    return;
                }
            };
        }
        cursor = listed_blobs.cursor.clone();
    }
    println!("Blobs successfully migrated!");

    let prefs = match current_agent
        .api
        .app
        .bsky
        .actor
        .get_preferences(get_preferences::ParametersData {}.into())
        .await
    {
        Ok(response) => response,
        Err(err) => {
            println!("app.bsky.actor.getPreferences at current PDS failed due to error: {err}");
            return;
        }
    };

    match new_agent
        .api
        .app
        .bsky
        .actor
        .put_preferences(
            put_preferences::InputData {
                preferences: prefs.preferences.clone(),
            }
            .into(),
        )
        .await
    {
        Ok(_) => (),
        Err(err) => {
            println!("app.bsky.actor.putPreferences at new PDS failed due to error: {err}");
            return;
        }
    }
    println!("Preferences successfully migrated!");
}
