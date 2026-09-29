//! Accounts and households, whichever way this Crumb signs people in: its own accounts
//! (`AUTH_MODE=accounts`, `/api/auth/*` answered by the Rust server) or the hosted
//! edition's Better Auth (`AUTH_MODE=hosted`, the same paths proxied to the auth service).
//! A port of `web/src/lib/account.ts`: [`Client::accounts`] gives one [`Accounts`] handle
//! for either, and the types here are the shapes that file's `Accounts` interface speaks.
//! Google, Apple and passkeys need a browser, so they aren't here (except listing and
//! unlinking the linked sign-ins). With one password, [`Client::login`] signs in.

use reqwest::Method;
use reqwest::header::ORIGIN;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Client, Download, Error, percent_encode_segment};

/// How this Crumb signs people in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// One app password.
    #[default]
    Password,
    /// Accounts the Rust server keeps.
    Accounts,
    /// Better Auth, in the hosted edition.
    Hosted,
}

/// A sign-in with a provider's own page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Google,
    Apple,
}

/// `GET /api/auth/status`: how this Crumb signs people in, and who's signed in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub mode: Mode,
    #[serde(default)]
    pub signed_in: bool,
    /// Accounts: nobody has an account yet, so the first one is made with [`Accounts::set_up`].
    #[serde(default)]
    pub setup_needed: bool,
    /// The setup asks for the old app password too.
    #[serde(default)]
    pub setup_needs_app_password: bool,
    #[serde(default)]
    pub signup_open: bool,
    /// One password: whether it's set (so there's a sign out).
    #[serde(default)]
    pub password_required: bool,
    /// Google and Apple, when this Crumb has their keys.
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub user: Option<StatusUser>,
    #[serde(default)]
    pub household: Option<StatusHousehold>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusUser {
    pub name: String,
    pub email: String,
}

/// The household the session works on. Its id is the server's own number, not the
/// string [`Household::id`] carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusHousehold {
    pub id: i64,
    pub name: String,
    pub role: String,
}

/// Someone in a household.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub you: bool,
}

/// An invite that hasn't been used yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingInvite {
    pub id: String,
    /// Hosted invites go to one address; self-hosted ones are links anyone can use once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    /// Unix seconds.
    pub expires_at: i64,
}

/// A household someone is in, for switching to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseholdRef {
    pub id: String,
    pub name: String,
}

/// The signed-in household: who's in it and what's waiting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Household {
    pub id: String,
    pub name: String,
    pub role: String,
    pub members: Vec<Member>,
    pub invites: Vec<PendingInvite>,
    /// Every household the person is in, this one included.
    pub households: Vec<HouseholdRef>,
}

/// A signed-in browser or app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub user_agent: Option<String>,
    /// Unix seconds.
    pub last_seen_at: i64,
    pub current: bool,
}

/// What an invite is for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitePreview {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub household_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invited_by: Option<String>,
    /// Hosted: the address the invite was sent to (only it can accept).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// A Google or Apple sign-in linked to the account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedProvider {
    pub provider: Provider,
    #[serde(default)]
    pub email: Option<String>,
    /// Unix seconds.
    pub created_at: i64,
}

/// How the account signs in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignInMethods {
    pub password: bool,
    pub linked: Vec<LinkedProvider>,
}

/// An app connected to Crumb (Claude, mostly): one per app and household.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedApp {
    pub id: String,
    pub name: String,
    pub household: Option<ConnectedHousehold>,
    /// Unix seconds.
    pub connected_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectedHousehold {
    pub id: i64,
    pub name: Option<String>,
}

/// Who is signing in or up. The name makes a new account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub email: String,
    pub password: String,
}

/// The result of making an account or joining with one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedUp {
    /// The new account must confirm its email before signing in.
    pub verify: bool,
}

/// A new invite: a link to hand over (hosted also emails it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InviteLink {
    pub url: String,
}

/// A Google or Apple sign-in started for an app: open `url` in the browser and keep
/// `verifier` for [`Accounts::finish_app_sign_in`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSignInStart {
    pub url: String,
    pub verifier: String,
}

/// `POST /api/account/email`: the new address, and whether it must be confirmed first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailChange {
    pub email: String,
    /// The new address was sent a link, and nothing changes until it's opened.
    pub pending: bool,
}

/// What a change-email link did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmailDone {
    Changed,
    Reverted,
}

/// `POST /api/account/email/confirm`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailLink {
    pub done: EmailDone,
    pub email: String,
}

/// The account calls that don't depend on the mode.
impl Client {
    /// `GET /api/auth/status`. Any failure reads as one password, as the web does.
    pub async fn auth_status(&self) -> Status {
        self.fetch(self.http.get(self.endpoint("api/auth/status")))
            .await
            .unwrap_or_default()
    }

    /// The account calls for `mode` (from [`Status::mode`]).
    pub fn accounts(&self, mode: Mode) -> Accounts<'_> {
        Accounts { client: self, mode }
    }

    /// `GET /api/connections`: the apps connected to this account.
    pub async fn connected_apps(&self) -> Result<Vec<ConnectedApp>, Error> {
        self.fetch(self.http.get(self.endpoint("api/connections")))
            .await
    }

    /// `DELETE /api/connections/{id}`: the app has to be approved again to reconnect.
    pub async fn disconnect_app(&self, id: &str) -> Result<(), Error> {
        self.call(
            self.http
                .delete(self.endpoint(&format!("api/connections/{}", percent_encode_segment(id)))),
        )
        .await
    }

    /// `GET /api/account/export`: the account, its devices and every household's recipes.
    pub async fn export_account(&self) -> Result<Download, Error> {
        self.download(self.http.get(self.endpoint("api/account/export")))
            .await
    }

    /// `POST /api/account/delete`: the account's password, or with none, its email typed
    /// out as `confirm`. Signs out. A wrong password is an [`Error::Api`] with status 401.
    pub async fn delete_account(
        &self,
        password: Option<&str>,
        confirm: Option<&str>,
    ) -> Result<(), Error> {
        // A wrong password is a 401 here, not a lapsed session
        let res = self
            .send(
                self.http
                    .post(self.endpoint("api/account/delete"))
                    .json(&json!({ "password": password, "confirm": confirm })),
            )
            .await?;
        self.expect_ok(res, true).await
    }

    /// `POST /api/account/email`: changes the account's email with its password, or with
    /// none, a recent sign-in.
    pub async fn change_email(&self, email: &str, password: &str) -> Result<EmailChange, Error> {
        let res = self
            .send(
                self.http
                    .post(self.endpoint("api/account/email"))
                    .json(&json!({ "email": email, "password": password })),
            )
            .await?;
        self.json(res, true).await
    }

    /// `POST /api/account/email/confirm`: a change-email link's token (hosted), which
    /// confirms the new address or puts the old one back.
    pub async fn confirm_email(&self, token: &str) -> Result<EmailLink, Error> {
        self.fetch(
            self.http
                .post(self.endpoint("api/account/email/confirm"))
                .json(&json!({ "token": token })),
        )
        .await
    }
}

/// The sign-in, household and device calls for one [`Mode`], from [`Client::accounts`].
/// Ids are strings whichever mode: Crumb's own numbers, or Better Auth's.
pub struct Accounts<'a> {
    client: &'a Client,
    mode: Mode,
}

impl Accounts<'_> {
    fn hosted(&self) -> bool {
        self.mode == Mode::Hosted
    }

    /// A request to `path`. Better Auth refuses a cookie's request that names no origin,
    /// as a browser's always does, so the hosted ones say theirs.
    fn req(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let req = self.client.http.request(method, self.client.endpoint(path));
        if self.hosted() {
            req.header(ORIGIN, self.client.url.origin().ascii_serialization())
        } else {
            req
        }
    }

    /// Sends `req`, keeping a 401 an ordinary error: it's a wrong password here.
    async fn attempt(&self, req: reqwest::RequestBuilder) -> Result<(), Error> {
        let res = self.client.send(req).await?;
        self.client.expect_ok(res, true).await
    }

    /// Sends `req` and takes a JSON body that may be `null` (signed out).
    async fn maybe<T: serde::de::DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<Option<T>, Error> {
        self.client.fetch(req).await
    }

    /// Signs in with an email and password; a wrong one is an [`Error::Api`] with status 401.
    pub async fn sign_in(&self, email: &str, password: &str) -> Result<(), Error> {
        let path = if self.hosted() {
            "api/auth/sign-in/email"
        } else {
            "api/auth/login"
        };
        self.attempt(
            self.req(Method::POST, path)
                .json(&json!({ "email": email, "password": password })),
        )
        .await
    }

    /// Signs in with Google or Apple (a [`Status::providers`] entry) through the browser, in
    /// either account mode: the browser comes back as an `app.crumb://signed-in?code=…` link
    /// (`crumb_core::app_link::signed_in_code`) for [`Accounts::finish_app_sign_in`].
    pub async fn start_app_sign_in(&self, provider: &str) -> Result<AppSignInStart, Error> {
        #[derive(Deserialize)]
        struct Started {
            url: String,
        }
        let pair = crumb_core::app_link::AppSignIn::new();
        let started: Started = self
            .client
            .fetch(self.req(Method::POST, "api/auth/app/start").json(&json!({
                "provider": provider, "challenge": pair.challenge,
            })))
            .await?;
        Ok(AppSignInStart {
            url: started.url,
            verifier: pair.verifier,
        })
    }

    /// Swaps the code the browser brought back for this client's own session.
    pub async fn finish_app_sign_in(&self, code: &str, verifier: &str) -> Result<(), Error> {
        self.attempt(
            self.req(Method::POST, "api/auth/app/redeem")
                .json(&json!({ "code": code, "verifier": verifier })),
        )
        .await
    }

    /// Accounts: makes the first account, which owns the recipe box already here.
    /// `app_password` is needed when [`Status::setup_needs_app_password`] says so.
    pub async fn set_up(
        &self,
        name: &str,
        email: &str,
        password: &str,
        app_password: Option<&str>,
    ) -> Result<(), Error> {
        self.attempt(self.req(Method::POST, "api/auth/setup").json(&json!({
            "name": name, "email": email, "password": password, "appPassword": app_password,
        })))
        .await
    }

    /// Makes an account, when sign-up is open.
    pub async fn sign_up(
        &self,
        name: &str,
        email: &str,
        password: &str,
    ) -> Result<SignedUp, Error> {
        self.sign_up_to(name, email, password, "/").await
    }

    /// [`Accounts::sign_up`], with the page (`callback`) a verification email leads to.
    async fn sign_up_to(
        &self,
        name: &str,
        email: &str,
        password: &str,
        callback: &str,
    ) -> Result<SignedUp, Error> {
        if !self.hosted() {
            let body = json!({ "name": name, "email": email, "password": password });
            self.client
                .call(self.req(Method::POST, "api/auth/signup").json(&body))
                .await?;
            return Ok(SignedUp { verify: false });
        }
        #[derive(Deserialize)]
        struct Made {
            token: Option<String>,
        }
        let made: Made = self
            .client
            .fetch(
                self.req(Method::POST, "api/auth/sign-up/email")
                    .json(&json!({
                        "name": name, "email": email, "password": password,
                        "callbackURL": callback,
                    })),
            )
            .await?;
        Ok(SignedUp {
            verify: made.token.is_none(),
        })
    }

    /// Signs out, here only.
    pub async fn sign_out(&self) -> Result<(), Error> {
        if self.hosted() {
            self.client
                .call(self.req(Method::POST, "api/auth/sign-out").json(&json!({})))
                .await
        } else {
            self.client.logout().await
        }
    }

    /// Where the account is signed in: this device first, then the most recent.
    pub async fn devices(&self) -> Result<Vec<Device>, Error> {
        if !self.hosted() {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Row {
                id: i64,
                user_agent: Option<String>,
                last_seen_at: i64,
                current: bool,
            }
            let rows: Vec<Row> = self
                .client
                .fetch(self.req(Method::GET, "api/auth/sessions"))
                .await?;
            return Ok(rows
                .into_iter()
                .map(|r| Device {
                    id: r.id.to_string(),
                    user_agent: r.user_agent,
                    last_seen_at: r.last_seen_at,
                    current: r.current,
                })
                .collect());
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Row {
            id: String,
            token: String,
            #[serde(default)]
            user_agent: Option<String>,
            #[serde(deserialize_with = "crumb_core::model::de_iso")]
            updated_at: i64,
        }
        let rows: Vec<Row> = self
            .client
            .fetch(self.req(Method::GET, "api/auth/list-sessions"))
            .await?;
        let current = self.current_session().await?.map(|s| s.session.id);
        let mut devices: Vec<Device> = rows
            .into_iter()
            .map(|r| Device {
                current: current.as_deref() == Some(r.id.as_str()),
                id: r.token,
                user_agent: r.user_agent,
                last_seen_at: r.updated_at,
            })
            .collect();
        devices.sort_by(|a, b| {
            b.current
                .cmp(&a.current)
                .then(b.last_seen_at.cmp(&a.last_seen_at))
        });
        Ok(devices)
    }

    /// Hosted: Better Auth's `get-session`, `null` when signed out.
    async fn current_session(&self) -> Result<Option<Current>, Error> {
        self.maybe(self.req(Method::GET, "api/auth/get-session"))
            .await
    }

    /// Signs out one device (a [`Device::id`]).
    pub async fn sign_out_device(&self, id: &str) -> Result<(), Error> {
        if self.hosted() {
            return self
                .client
                .call(
                    self.req(Method::POST, "api/auth/revoke-session")
                        .json(&json!({ "token": id })),
                )
                .await;
        }
        let path = format!("api/auth/sessions/{}", percent_encode_segment(id));
        self.client.call(self.req(Method::DELETE, &path)).await
    }

    /// Signs out every device but this one.
    pub async fn sign_out_others(&self) -> Result<(), Error> {
        if self.hosted() {
            self.client
                .call(
                    self.req(Method::POST, "api/auth/revoke-other-sessions")
                        .json(&json!({})),
                )
                .await
        } else {
            self.client
                .call(self.req(Method::POST, "api/auth/sessions/revoke-others"))
                .await
        }
    }

    /// The signed-in household: its members, waiting invites and the person's others.
    pub async fn household(&self) -> Result<Household, Error> {
        if !self.hosted() {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Row {
                id: i64,
                name: String,
                role: String,
                you: i64,
                members: Vec<RowMember>,
                invites: Vec<RowInvite>,
                households: Vec<RowRef>,
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct RowMember {
                user_id: i64,
                name: String,
                email: String,
                role: String,
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct RowInvite {
                id: i64,
                created_by: Option<String>,
                expires_at: i64,
            }
            #[derive(Deserialize)]
            struct RowRef {
                id: i64,
                name: String,
            }
            let h: Row = self
                .client
                .fetch(self.req(Method::GET, "api/auth/household"))
                .await?;
            return Ok(Household {
                id: h.id.to_string(),
                name: h.name,
                role: h.role,
                members: h
                    .members
                    .into_iter()
                    .map(|m| Member {
                        you: m.user_id == h.you,
                        id: m.user_id.to_string(),
                        name: m.name,
                        email: m.email,
                        role: m.role,
                    })
                    .collect(),
                invites: h
                    .invites
                    .into_iter()
                    .map(|i| PendingInvite {
                        id: i.id.to_string(),
                        email: None,
                        created_by: i.created_by,
                        expires_at: i.expires_at,
                    })
                    .collect(),
                households: h
                    .households
                    .into_iter()
                    .map(|o| HouseholdRef {
                        id: o.id.to_string(),
                        name: o.name,
                    })
                    .collect(),
            });
        }
        #[derive(Deserialize)]
        struct Org {
            id: String,
            name: String,
            members: Vec<OrgMember>,
            invitations: Vec<OrgInvite>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct OrgMember {
            id: String,
            user_id: String,
            role: String,
            user: OrgUser,
        }
        #[derive(Deserialize)]
        struct OrgUser {
            name: String,
            email: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct OrgInvite {
            id: String,
            email: String,
            status: String,
            #[serde(deserialize_with = "crumb_core::model::de_iso")]
            expires_at: i64,
        }
        let org: Org = self
            .client
            .fetch(self.req(Method::GET, "api/auth/organization/get-full-organization"))
            .await?;
        let orgs: Vec<HouseholdRef> = self
            .client
            .fetch(self.req(Method::GET, "api/auth/organization/list"))
            .await?;
        let me = self.current_session().await?.map(|s| s.user.id);
        let role = org
            .members
            .iter()
            .find(|m| Some(&m.user_id) == me.as_ref())
            .map_or("member", |m| m.role.as_str())
            .to_string();
        let mut members: Vec<Member> = org
            .members
            .into_iter()
            .map(|m| Member {
                you: Some(&m.user_id) == me.as_ref(),
                id: m.id,
                name: m.user.name,
                email: m.user.email,
                role: m.role,
            })
            .collect();
        // Owners first, and no one else moves
        members.sort_by_key(|m| m.role != "owner");
        let now = now_secs();
        Ok(Household {
            id: org.id,
            name: org.name,
            role,
            members,
            invites: org
                .invitations
                .into_iter()
                .filter(|i| i.status == "pending" && i.expires_at > now)
                .map(|i| PendingInvite {
                    id: i.id,
                    email: Some(i.email),
                    created_by: None,
                    expires_at: i.expires_at,
                })
                .collect(),
            households: orgs,
        })
    }

    /// Renames the signed-in household.
    pub async fn rename(&self, name: &str) -> Result<(), Error> {
        if self.hosted() {
            self.client
                .call(
                    self.req(Method::POST, "api/auth/organization/update")
                        .json(&json!({ "data": { "name": name } })),
                )
                .await
        } else {
            self.client
                .call(
                    self.req(Method::PATCH, "api/auth/household")
                        .json(&json!({ "name": name })),
                )
                .await
        }
    }

    /// Invites someone. Self-hosted: a link to hand over (`email` is unused). Hosted: an
    /// email goes to `email`, and the link comes back too.
    pub async fn invite(&self, email: Option<&str>) -> Result<InviteLink, Error> {
        if !self.hosted() {
            return self
                .client
                .fetch(self.req(Method::POST, "api/auth/invites"))
                .await;
        }
        #[derive(Deserialize)]
        struct Made {
            id: String,
        }
        let made: Made = self
            .client
            .fetch(
                self.req(Method::POST, "api/auth/organization/invite-member")
                    .json(&json!({ "email": email, "role": "member" })),
            )
            .await?;
        Ok(InviteLink {
            url: format!("{}/invite#{}", self.client.base, made.id),
        })
    }

    /// Cancels a waiting invite (a [`PendingInvite::id`]).
    pub async fn cancel_invite(&self, id: &str) -> Result<(), Error> {
        if self.hosted() {
            return self
                .client
                .call(
                    self.req(Method::POST, "api/auth/organization/cancel-invitation")
                        .json(&json!({ "invitationId": id })),
                )
                .await;
        }
        let path = format!("api/auth/invites/{}", percent_encode_segment(id));
        self.client.call(self.req(Method::DELETE, &path)).await
    }

    /// Takes a member (a [`Member::id`]) out of the household.
    pub async fn remove_member(&self, id: &str) -> Result<(), Error> {
        if self.hosted() {
            return self
                .client
                .call(
                    self.req(Method::POST, "api/auth/organization/remove-member")
                        .json(&json!({ "memberIdOrEmail": id })),
                )
                .await;
        }
        let path = format!("api/auth/members/{}", percent_encode_segment(id));
        self.client.call(self.req(Method::DELETE, &path)).await
    }

    /// Leaves the household `id` (the signed-in one, [`Household::id`]). The server picks
    /// the next one, or makes one.
    pub async fn leave(&self, id: &str) -> Result<(), Error> {
        if self.hosted() {
            self.client
                .call(
                    self.req(Method::POST, "api/auth/organization/leave")
                        .json(&json!({ "organizationId": id })),
                )
                .await
        } else {
            self.client
                .call(self.req(Method::POST, "api/auth/household/leave"))
                .await
        }
    }

    /// Works on another of the person's households (a [`HouseholdRef::id`]).
    pub async fn switch_to(&self, id: &str) -> Result<(), Error> {
        if self.hosted() {
            return self
                .client
                .call(
                    self.req(Method::POST, "api/auth/organization/set-active")
                        .json(&json!({ "organizationId": id })),
                )
                .await;
        }
        let id: i64 = id.parse().map_err(|_| Error::Api {
            status: 400,
            message: "That isn't a household".into(),
            code: None,
            site: None,
        })?;
        self.client
            .call(
                self.req(Method::POST, "api/auth/household/switch")
                    .json(&json!({ "id": id })),
            )
            .await
    }

    /// What an invite (its link's token) is for, when it can still be used. Hosted, only
    /// once signed in.
    pub async fn preview_invite(&self, token: &str) -> Result<Option<InvitePreview>, Error> {
        if !self.hosted() {
            let res = self
                .client
                .fetch(
                    self.req(Method::POST, "api/auth/invite/preview")
                        .json(&json!({ "token": token })),
                )
                .await;
            return Ok(res.ok());
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Invite {
            organization_name: String,
            inviter_email: String,
            email: String,
        }
        let res: Result<Invite, Error> = self
            .client
            .fetch(
                self.req(Method::GET, "api/auth/organization/get-invitation")
                    .query(&[("id", token)]),
            )
            .await;
        Ok(res.ok().map(|i| InvitePreview {
            household_name: Some(i.organization_name),
            invited_by: Some(i.inviter_email),
            email: Some(i.email),
        }))
    }

    /// Joins by an invite's token. Signed out, `creds` make an account (with a name) or
    /// sign in to one first.
    pub async fn accept_invite(
        &self,
        token: &str,
        creds: Option<&Credentials>,
    ) -> Result<SignedUp, Error> {
        if !self.hosted() {
            let mut body = json!(creds);
            if body.is_null() {
                body = json!({});
            }
            body["token"] = json!(token);
            self.client
                .call(self.req(Method::POST, "api/auth/invite/accept").json(&body))
                .await?;
            return Ok(SignedUp { verify: false });
        }
        if let Some(creds) = creds {
            match creds.name.as_deref().filter(|n| !n.is_empty()) {
                Some(name) => {
                    let callback = format!("/invite#{token}");
                    let made = self
                        .sign_up_to(name, &creds.email, &creds.password, &callback)
                        .await?;
                    if made.verify {
                        return Ok(made);
                    }
                }
                None => self.sign_in(&creds.email, &creds.password).await?,
            }
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Joined {
            member: JoinedMember,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct JoinedMember {
            organization_id: String,
        }
        let joined: Joined = self
            .client
            .fetch(
                self.req(Method::POST, "api/auth/organization/accept-invitation")
                    .json(&json!({ "invitationId": token })),
            )
            .await?;
        self.switch_to(&joined.member.organization_id).await?;
        Ok(SignedUp { verify: false })
    }

    /// Hosted: sends a password reset link to `email`. Not available with own accounts.
    pub async fn request_reset(&self, email: &str) -> Result<(), Error> {
        self.hosted_only()?;
        self.client
            .call(
                self.req(Method::POST, "api/auth/request-password-reset")
                    .json(&json!({ "email": email, "redirectTo": "/reset-password" })),
            )
            .await
    }

    /// Hosted: sets a new password from a reset link's token.
    pub async fn reset_password(&self, token: &str, password: &str) -> Result<(), Error> {
        self.hosted_only()?;
        self.client
            .call(
                self.req(Method::POST, "api/auth/reset-password")
                    .json(&json!({ "token": token, "newPassword": password })),
            )
            .await
    }

    /// Whether this mode can reset a password by email ([`Accounts::request_reset`]).
    pub fn can_reset_password(&self) -> bool {
        self.hosted()
    }

    fn hosted_only(&self) -> Result<(), Error> {
        if self.hosted() {
            Ok(())
        } else {
            Err(Error::Api {
                status: 404,
                message: "Passwords can't be reset by email here".into(),
                code: None,
                site: None,
            })
        }
    }

    /// How the account signs in: with a password, and which Google or Apple sign-ins are
    /// linked.
    pub async fn sign_in_methods(&self) -> Result<SignInMethods, Error> {
        if !self.hosted() {
            return self
                .client
                .fetch(self.req(Method::GET, "api/auth/identities"))
                .await;
        }
        let rows = self.linked_accounts().await?;
        Ok(SignInMethods {
            password: rows.iter().any(|r| r.provider_id == "credential"),
            linked: rows
                .iter()
                .filter_map(|r| {
                    let provider = match r.provider_id.as_str() {
                        "google" => Provider::Google,
                        "apple" => Provider::Apple,
                        _ => return None,
                    };
                    Some(LinkedProvider {
                        provider,
                        email: None,
                        created_at: r.created_at,
                    })
                })
                .collect(),
        })
    }

    /// Hosted: Better Auth's `list-accounts`.
    async fn linked_accounts(&self) -> Result<Vec<Linked>, Error> {
        self.client
            .fetch(self.req(Method::GET, "api/auth/list-accounts"))
            .await
    }

    /// Unlinks a Google or Apple sign-in.
    pub async fn unlink(&self, provider: Provider) -> Result<(), Error> {
        let name = match provider {
            Provider::Google => "google",
            Provider::Apple => "apple",
        };
        if !self.hosted() {
            return self
                .client
                .call(self.req(Method::DELETE, &format!("api/auth/identities/{name}")))
                .await;
        }
        let Some(row) = self
            .linked_accounts()
            .await?
            .into_iter()
            .find(|r| r.provider_id == name)
        else {
            return Ok(());
        };
        self.client
            .call(
                self.req(Method::POST, "api/auth/unlink-account")
                    .json(&json!({ "accountId": row.id })),
            )
            .await
    }
}

/// Better Auth's `get-session`.
#[derive(Deserialize)]
struct Current {
    session: CurrentSession,
    user: CurrentUser,
}

#[derive(Deserialize)]
struct CurrentSession {
    id: String,
}

#[derive(Deserialize)]
struct CurrentUser {
    id: String,
}

/// A row of Better Auth's `list-accounts`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Linked {
    id: String,
    provider_id: String,
    #[serde(deserialize_with = "crumb_core::model::de_iso")]
    created_at: i64,
}

fn now_secs() -> i64 {
    crumb_core::model::now_secs()
}
