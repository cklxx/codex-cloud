# Codex Cloud Backend

This service exposes the HTTP API that powers Codex Cloud. It can authenticate
users locally or via an external OpenID Connect (OIDC) identity provider.

## OpenID Connect configuration

OIDC support is optional. When the following environment variables are present
at start-up, the backend will perform the OAuth 2.0 authorization-code flow
against the configured provider and issue Codex sessions for identities that are
linked to local users:

| Variable | Required | Description |
| --- | --- | --- |
| `CODEX_OIDC_ISSUER` | Yes | Base issuer URL of the OIDC provider (for example `https://dev-1234.okta.com`) used for discovery and issuer validation. |
| `CODEX_OIDC_CLIENT_ID` | Yes | Client identifier provisioned for Codex in the OIDC provider. |
| `CODEX_OIDC_CLIENT_SECRET` | Yes | Client secret associated with the client identifier. |
| `CODEX_OIDC_REDIRECT_URI` | No | Callback URL registered with the provider. Defaults to `http://localhost:8000/auth/oidc/callback`. |
| `CODEX_OIDC_JWKS_CACHE_TTL` | No | Maximum age (seconds) to keep keys from the provider JWKS endpoint. Defaults to 3600 seconds. |
| `CODEX_OIDC_JWKS_CACHE_REFRESH` | No | Interval (seconds) after which keys are refreshed opportunistically. Defaults to 300 seconds. |

The backend validates the issuer reported during discovery and ID token
validation. Only RSA-signed tokens (RS256/RS384/RS512) are accepted. Tokens are
cached according to the TTL/refresh settings above.

### Provisioning an OIDC client for local Docker Compose

1. In your identity provider (Auth0, Okta, Azure AD, etc.) create a new
   confidential client application.
2. Register the callback URL `http://localhost:8000/auth/oidc/callback` for the
   new client.
3. Create a `.env` file next to `cloud/docker-compose.yml` and populate the
   client credentials, for example:

   ```env
   CODEX_OIDC_ISSUER=https://your-tenant.example.com/oidc
   CODEX_OIDC_CLIENT_ID=codex-local
   CODEX_OIDC_CLIENT_SECRET=super-secret
   CODEX_OIDC_REDIRECT_URI=http://localhost:8000/auth/oidc/callback
   CODEX_OIDC_JWKS_CACHE_TTL=3600
   CODEX_OIDC_JWKS_CACHE_REFRESH=300
   ```

4. Start the stack with `docker compose up`. The backend container will read the
   variables above and enable the OIDC login callback at
   `/auth/oidc/callback`.

### Linking external identities to local users

OIDC logins only succeed when the external identity has been mapped to an
existing Codex user. The database now includes an `external_identities` table
for this mapping along with a helper for seeding records programmatically.

1. Create a local user (for example with `cargo run -p codex-cloud-backend --
   create-admin user@example.com <password>`).
2. Insert or seed an external identity record that links the provider issuer
   and subject to that user. Application code or one-off scripts can call
   `db::seed_external_identities`:

   ```rust
   use codex_cloud_backend::db::{self, ExternalIdentitySeed};
   use uuid::Uuid;

   db::seed_external_identities(
       &pool,
       &[ExternalIdentitySeed {
           issuer: "https://your-tenant.example.com/oidc",
           subject: "00u123example",
           user_id: Uuid::parse_str("...").unwrap(),
           email: Some("user@example.com"),
       }],
   )
   .await?;
   ```

   Alternatively you can execute an `INSERT` into the `external_identities`
   table directly using your database tooling.

Once a mapping exists, successful OIDC logins will exchange the authorization
code for an ID token, validate it against the provider JWKS keys, and issue a
Codex access token for the linked user.
