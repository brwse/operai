# Authentication Guide

This document describes the authentication patterns used by operai integrations and how to configure credentials for different services.

## Credential Types

operai supports two types of credentials:

| Type       | Macro                       | Configuration                | Use Case                                    |
| ---------- | --------------------------- | ---------------------------- | ------------------------------------------- |
| **System** | `define_system_credential!` | Manifest (`operai.toml`)     | Static credentials configured per-deployment |

### Choosing Between User and System Credentials

| Choose...                   | When...                                                                                  |
| --------------------------- | ---------------------------------------------------------------------------------------- |
| `define_user_credential!`   | Credentials may vary per-request (different users, different accounts, ephemeral tokens) |
| `define_system_credential!` | Credentials are static and configured in the manifest                    |

The choice depends on the deployment model. For example:
- **API Key**: System if one key for the whole service, User if each request may use a different key
- **AWS Credentials**: System if tool uses a specific role, User if accessing different AWS accounts per-request

---

## Authentication Patterns

### 1. OAuth2 Bearer Token

**Typical Credential Type**: User (ephemeral, refreshable tokens)

**User Journey:**
1. User obtains access token through their app's OAuth flow (outside operai)
2. Token is passed per-request via gRPC headers
3. operai reads token from `Context` via `define_user_credential!`

**Services**: Gmail, Outlook, Slack, GitHub, Google Calendar, Salesforce, HubSpot, etc.

**Credential Schema:**
```rust
define_user_credential! {
    ServiceCredential("service_name") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,  // For custom/self-hosted endpoints
    }
}
```

**Usage in Tool:**
```rust
#[tool(...)]
pub async fn my_tool(ctx: Context, input: MyInput) -> Result<MyOutput> {
    let cred = ServiceCredential::get(&ctx)?;

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.service.com/endpoint")
        .bearer_auth(&cred.access_token)
        .send()
        .await?;
    // ...
}
```

---

### 2. API Key

**Typical Credential Type**: System (static, configured at deployment)

**User Journey:**
1. Operator adds API key to `operai.toml`
2. operai reads from manifest at startup

**Services**: SendGrid, Twilio, Stripe, PagerDuty, Datadog, New Relic, Sentry, etc.

**Manifest Configuration:**
```toml
[[tools]]
package = "my-tool"
[tools.credentials.service_name]
api_key = "..."
# endpoint = "..." (optional)
```

**Credential Schema:**
```rust
define_system_credential! {
    ServiceCredential("service_name") {
        api_key: String,
        #[optional]
        endpoint: Option<String>,
    }
}
```

**Usage in Tool:**
```rust
#[tool(...)]
pub async fn my_tool(ctx: Context, input: MyInput) -> Result<MyOutput> {
    let cred = ServiceCredential::get(&ctx)?;

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.service.com/endpoint")
        .header("Authorization", format!("Bearer {}", cred.api_key))
        // Or: .header("X-Api-Key", &cred.api_key)
        .send()
        .await?;
    // ...
}
```

---

### 3. AWS Signature V4

**Typical Credential Type**: System (service-level IAM credentials)

**User Journey:**
1. Operator sets credentials in `operai.toml`
2. Optional: session token for temporary credentials (STS)
3. Region configured per-deployment

**Services**: All AWS services (S3, EC2, Lambda, DynamoDB, SQS, SNS, etc.)

**Manifest Configuration:**
```toml
[[tools]]
package = "aws-tool"
[tools.credentials.aws]
access_key_id = "..."
secret_access_key = "..."
region = "us-east-1"
# session_token = "..." (optional)
# endpoint = "..." (optional)
```

**Credential Schema:**
```rust
define_system_credential! {
    AwsCredential("aws") {
        access_key_id: String,
        secret_access_key: String,
        region: String,
        #[optional]
        session_token: Option<String>,
        #[optional]
        endpoint: Option<String>,
    }
}
```

**Usage in Tool:**

Use the official AWS SDK for Rust (`aws-sdk-*` crates) which handles SigV4 signing automatically:

```rust
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;

#[tool(...)]
pub async fn list_buckets(ctx: Context, input: ListBucketsInput) -> Result<ListBucketsOutput> {
    let cred = AwsCredential::get(&ctx)?;

    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new(cred.region.clone()))
        .credentials_provider(aws_credential_types::Credentials::new(
            &cred.access_key_id,
            &cred.secret_access_key,
            cred.session_token.clone(),
            None,
            "operai",
        ))
        .load()
        .await;

    let client = Client::new(&config);
    let result = client.list_buckets().send().await?;
    // ...
}
```

---

### 4. GCP Service Account

**Typical Credential Type**: System (static service account JSON)

**User Journey:**
1. Operator downloads service account JSON from GCP Console
2. JSON content set in `operai.toml`
3. operai reads and parses at startup

**Services**: All GCP services (Cloud Storage, BigQuery, Compute Engine, etc.)

**Manifest Configuration:**
```toml
[[tools]]
package = "gcp-tool"
[tools.credentials.gcp]
service_account_json = "..."
# project_id = "..." (optional)
```

**Credential Schema:**
```rust
define_system_credential! {
    GcpCredential("gcp") {
        service_account_json: String,
        #[optional]
        project_id: Option<String>,
    }
}
```

**Usage in Tool:**

Use the official Google Cloud SDK crates (`google-cloud-*`):

```rust
use google_cloud_storage::client::{Client, ClientConfig};

#[tool(...)]
pub async fn list_buckets(ctx: Context, input: ListBucketsInput) -> Result<ListBucketsOutput> {
    let cred = GcpCredential::get(&ctx)?;

    let config = ClientConfig::default()
        .with_credentials_json(&cred.service_account_json)
        .await?;

    let client = Client::new(config);
    let buckets = client.list_buckets(&cred.project_id.unwrap_or_default()).await?;
    // ...
}
```

---

### 5. Azure Service Principal

**Typical Credential Type**: System (static client credentials)

**User Journey:**
1. Operator creates service principal in Azure AD
2. Credentials set in `operai.toml`
3. Tool uses client credentials flow to obtain tokens

**Services**: All Azure services (Blob Storage, VMs, Functions, Key Vault, etc.)

**Manifest Configuration:**
```toml
[[tools]]
package = "azure-tool"
[tools.credentials.azure]
tenant_id = "..."
client_id = "..."
client_secret = "..."
# subscription_id = "..." (optional)
```

**Credential Schema:**
```rust
define_system_credential! {
    AzureCredential("azure") {
        tenant_id: String,
        client_id: String,
        client_secret: String,
        #[optional]
        subscription_id: Option<String>,
    }
}
```

**Usage in Tool:**

Use the Azure SDK for Rust (`azure_*` crates):

```rust
use azure_identity::ClientSecretCredential;
use azure_storage_blobs::prelude::*;

#[tool(...)]
pub async fn list_containers(ctx: Context, input: ListContainersInput) -> Result<ListContainersOutput> {
    let cred = AzureCredential::get(&ctx)?;

    let credential = ClientSecretCredential::new(
        cred.tenant_id.clone(),
        cred.client_id.clone(),
        cred.client_secret.clone(),
    );

    let client = BlobServiceClient::new(&input.account_name, credential);
    let containers = client.list_containers().await?;
    // ...
}
```

---

### 6. Basic Auth

**Typical Credential Type**: System (static username/password)

**User Journey:**
1. Operator configures username + API token in `operai.toml`

**Services**: Jira, Jenkins, Bitbucket, some self-hosted services

**Manifest Configuration:**
```toml
[[tools]]
package = "my-tool"
[tools.credentials.service_name]
username = "..."
password = "..."
# endpoint = "..." (optional)
```

**Credential Schema:**
```rust
define_system_credential! {
    ServiceCredential("service_name") {
        username: String,
        password: String,  // Often an API token, not actual password
        #[optional]
        endpoint: Option<String>,
    }
}
```

**Usage in Tool:**
```rust
#[tool(...)]
pub async fn my_tool(ctx: Context, input: MyInput) -> Result<MyOutput> {
    let cred = ServiceCredential::get(&ctx)?;
    let endpoint = cred.endpoint.as_deref().unwrap_or("https://api.service.com");

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/endpoint", endpoint))
        .basic_auth(&cred.username, Some(&cred.password))
        .send()
        .await?;
    // ...
}
```

---

### 7. IMAP/SMTP

**Typical Credential Type**: System (static mail server configuration)

**User Journey:**
1. Operator configures mail server credentials in `operai.toml`
2. Tool uses these credentials for all email operations

**Services**: Generic IMAP/SMTP integration

**Manifest Configuration:**
```toml
[[tools]]
package = "mail-tool"
[tools.credentials.imap_smtp]
username = "..."
password = "..."
imap_server = "imap.example.com"
smtp_server = "smtp.example.com"
# imap_port = "993" (optional)
# smtp_port = "587" (optional)
```

**Credential Schema:**
```rust
define_system_credential! {
    ImapSmtpCredential("imap_smtp") {
        username: String,
        password: String,
        imap_server: String,
        smtp_server: String,
        #[optional]
        imap_port: Option<u16>,
        #[optional]
        smtp_port: Option<u16>,
    }
}
```

**Usage in Tool:**
```rust
use async_imap::Client;
use async_native_tls::TlsConnector;

#[tool(...)]
pub async fn fetch_messages(ctx: Context, input: FetchMessagesInput) -> Result<FetchMessagesOutput> {
    let cred = ImapSmtpCredential::get(&ctx)?;
    let port = cred.imap_port.unwrap_or(993);

    let tls = TlsConnector::new();
    let client = Client::connect((&*cred.imap_server, port), &cred.imap_server, tls).await?;
    let mut session = client.login(&cred.username, &cred.password).await?;
    // ...
}
```

---

## Default Recommendations by Auth Type

| Auth Type               | Default Credential | Rationale                                  |
| ----------------------- | ------------------ | ------------------------------------------ |
| OAuth2 Bearer           | User               | Tokens are typically ephemeral/refreshable |
| API Key                 | System             | Usually one key per deployment             |
| Basic Auth              | System             | Usually static credentials                 |
| AWS Signature V4        | System             | Usually service-level IAM                  |
| GCP Service Account     | System             | Usually one service account                |
| Azure Service Principal | System             | Usually one service principal              |
| IMAP/SMTP               | System             | Usually one mail server config             |

---

## Integration → Auth Type Mapping

### OAuth2 Bearer Token Services
Gmail, Outlook Mail, Google Calendar, Outlook Calendar, Slack, Microsoft Teams, Google Chat, Zoom, Notion, Confluence, Google Docs, Google Sheets, Google Drive, OneDrive, SharePoint, Dropbox, Box, GitHub, GitLab, Salesforce, HubSpot, Zendesk, Linear, Asana, Trello, Figma, etc.

### API Key Services
SendGrid, Postmark, Mailgun, Twilio, Stripe, PagerDuty, Datadog, New Relic, Sentry, LaunchDarkly, Vercel, Netlify, Airtable, etc.

### Basic Auth Services
Jira, Bitbucket, Jenkins, Confluence (some deployments), etc.

### AWS Signature V4 Services
S3, EC2, Lambda, DynamoDB, SQS, SNS, CloudWatch, IAM, RDS, EKS, ECS, Route 53, Secrets Manager, KMS, etc.

### GCP Service Account Services
Cloud Storage, Compute Engine, BigQuery, Cloud Run, Cloud Functions, Pub/Sub, Cloud SQL, GKE, etc.

### Azure Service Principal Services
Blob Storage, Virtual Machines, Functions, Key Vault, Cosmos DB, AKS, Event Grid, Service Bus, etc.
