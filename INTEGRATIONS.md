# Potential Integrations for operai-tool

Each line is `Integration — suggested tiny tools (verbs)`.

Principles:
- Tools stay narrow (small + many) and split per product (e.g., Gmail ≠ “Google”).
- Prefer a handful of high-value verbs per integration (often 3–8).

## Email & Inbox
- Outlook Mail (Microsoft 365) — search mail; get message; send/reply; move folder; flag

## CI/CD & Build
- Azure Pipelines — run pipeline; get status; fetch logs; approve stage
- TeamCity — trigger build; status; fetch logs; download artifacts

## Deploy & Release
- Argo CD — sync app; get health/status; view diff; rollback
- Spinnaker — trigger deployment; pipeline status; rollback; view logs
- Harness — trigger pipeline; approve step; get status; fetch logs
- Octopus Deploy — create release; deploy; get status; rollback
- Vercel — list deployments; create deployment; get logs; promote/rollback
- Netlify — trigger deploy; list deploys; get logs; rollback
- Render — deploy service; get deploy status; fetch logs; restart service
- Fly.io — deploy app; scale; fetch logs; rollback
- Heroku — create release; rollback; config vars; dyno restart

## Infrastructure as Code & Ops Automation
- Terraform Cloud — start plan; apply run; get output; lock/unlock workspace
- Pulumi Cloud — preview; apply; fetch stack outputs; list deployments
- CloudFormation — create/update stack; get events; rollback; delete stack
- Ansible Tower/AWX — launch job; get status; fetch stdout; set vars
- Rundeck — run job; execution status; fetch logs; approve step

## Kubernetes & Containers
- Kubernetes (generic) — list workloads; get pod logs; rollout restart; scale; describe resource
- Docker Hub — list repos/tags; get tag metadata; delete tag; webhooks
- AWS ECR — list repos/images; scan findings; delete image; lifecycle policy
- GCP Artifact Registry — list repos; list images; scan results; delete version
- Azure Container Registry — list repos/tags; delete tag; task runs; scan results
- Harbor — list projects/repos; scan results; promote image; delete artifact

## Artifacts & Registries
- JFrog Artifactory — search artifacts; download; publish; manage repo permissions
- Sonatype Nexus — search artifacts; download; publish; manage repo
- npm registry — publish; deprecate version; manage dist-tags
- PyPI — upload; yank version; release metadata
- GitHub Packages — publish; list versions; delete version

## Cloud Providers (Service-Level)
- AWS S3 — list buckets; list objects; get object; put object; generate presigned URL
- AWS EC2 — list instances; start/stop; get console output; tag resource
- AWS Lambda — invoke function; list functions; get configuration; update env vars
- AWS IAM — list users/roles; get policies; simulate policy; audit access (read-only)
- AWS CloudWatch — query metrics; run Logs Insights; list alarms; set alarm state
- AWS CloudTrail — lookup events; get trail status; export recent events
- AWS SNS — publish message; list topics; manage subscriptions
- AWS SQS — send message; receive/delete message; get queue attributes
- AWS DynamoDB — get item; put item; query; update item
- AWS RDS — list instances; snapshot; restore (guarded); parameter group status
- AWS EKS — list clusters; describe cluster; update nodegroup; get kubeconfig (link)
- AWS ECS — list services; update service; view tasks; fetch task logs link
- AWS Route 53 — list zones; upsert record; health check status
- AWS Secrets Manager — get secret value; list secrets; rotate secret (guarded)
- AWS KMS — list keys; encrypt/decrypt (guarded); key policy summary
- AWS Cost Explorer — daily cost report; anomaly summary; forecast
- AWS Organizations — list accounts; SCP summary; org structure
- AWS Systems Manager (SSM) — run command; get command output; patch compliance

- GCP Cloud Storage — list buckets; list objects; get object; upload object; signed URL
- GCP Compute Engine — list instances; start/stop; serial console output; labels
- GCP Cloud Run — list services; deploy revision; traffic split; logs link
- GCP Cloud Functions — list functions; invoke; update env vars; logs link
- GCP IAM — list principals; policy bindings; audit logs query (read-only)
- GCP Cloud Logging — query logs; saved queries; export sink status
- GCP Cloud Monitoring — query metrics; alert policies; mute/unmute
- GCP Pub/Sub — publish; pull/ack; subscriptions; topic config
- GCP Secret Manager — access secret; list secrets; rotate secret (guarded)
- GCP BigQuery — run query; job status; export table; dataset/table metadata
- GCP GKE — list clusters; describe; node pool ops; credentials helper
- GCP Cloud SQL — list instances; start/stop; backups; user list
- GCP Cloud Build — trigger build; build status; logs; artifacts
- GCP Cloud DNS — list zones; upsert record; DNSSEC status

- Azure Blob Storage — list containers; list blobs; download/upload; SAS link
- Azure Virtual Machines — list VMs; start/stop; run command; tags
- Azure Functions — list functions; invoke; app settings; logs link
- Azure Key Vault — get secret; list secrets; set secret (guarded); access policy summary
- Azure Monitor — query metrics; alerts; action groups; mute/unmute
- Azure Log Analytics — run KQL; saved queries; export results
- Azure Event Grid — publish event; list subscriptions; dead-letter status
- Azure Service Bus — send; receive/complete; queue/topic status
- Azure Cosmos DB — query; read/write item; throughput status
- Azure AKS — list clusters; describe; node pool ops; credentials helper

## Edge, DNS & CDN
- Cloudflare DNS — list zones; upsert records; purge cache; analytics summary
- Cloudflare Workers — deploy worker; list routes; tail logs; KV ops
- Cloudflare R2 — list buckets; get/put object; presigned URL
- Fastly — purge URL; purge service; list configs; stats snapshot
- Akamai — purge cache; property status; edge logs link
- Route 53 (AWS) — list zones; upsert record; health check status

## Observability (Logs/Metrics/Tracing)
- Datadog — query metrics; search monitors; mute/unmute; query logs; create annotation
- New Relic — run NRQL; list alerts; acknowledge; create annotation
- Grafana — query dashboards; create annotation; manage alert rules
- Prometheus — run query; list rules; fetch series
- Alertmanager — list alerts; create silence; expire silence; routing info
- Sentry — search issues; get issue; comment; resolve/reopen; create release
- Elastic/Kibana — run KQL; fetch logs; manage alert rules; open case link
- CloudWatch Logs — run Logs Insights; get results; alarms
- Honeycomb — query; dataset list; create marker; fetch trace link

## Incident Response & On-call
- PagerDuty — trigger; acknowledge; resolve; add note; page on-call
- Opsgenie — create alert; acknowledge; close; add note; schedule override
- FireHydrant — create incident; update status; add timeline event; postmortem tasks
- Rootly — create incident; assign roles; update Slack channel; post update
- Statuspage — create/update incident; update components; resolve incident
- Incident.io — create incident; update timeline; assign roles; status update

## ITSM & Internal Support
- ServiceNow — search incidents/requests; get record; comment; update state; assign
- Freshservice — search tickets; reply/note; assign; change status; create problem
- ManageEngine ServiceDesk — search requests; update; comment; assign

## SecOps (SIEM/SOAR/EDR)
- Microsoft Sentinel — run KQL; list incidents; update status; assign owner; comment
- Splunk ES — run SPL; list notable events; assign/close; comment
- IBM QRadar — search offenses; get offense; assign; close; add note
- Elastic Security — list alerts; add to case; update status; add note
- Cortex XSOAR — list incidents; run playbook; add note; close incident
- TheHive — create case; add observable; add task; close case
- CrowdStrike — search detections; contain host; query IOC; case notes
- SentinelOne — list threats; remediate; isolate endpoint; add note
- Wiz — list findings; filter by asset; create exception; mark resolved; export
- Lacework — list alerts; acknowledge; suppress; export

## Vulnerability & AppSec
- Snyk — list projects; get issues; ignore issue; create ticket; summary report
- SonarQube — project status; list issues; assign; mark resolved; export report
- Veracode — list scans; get findings; mitigation; export report
- Checkmarx — list findings; triage; create ticket; export report
- Tenable — list scans; export findings; ticket sync; exception workflow
- Qualys — list vulnerabilities; export report; exception workflow; tag assets

## Cloud Security Posture
- AWS Security Hub — list findings; update workflow status; create insight; link ticket
- AWS GuardDuty — list findings; get finding; archive/unarchive
- Defender for Cloud — list recommendations; set status; export; create ticket

## Identity & Access (IAM)
- Okta — search users; deactivate/reactivate; reset MFA; group membership; audit events
- Entra ID (Azure AD) — search users/groups; add/remove member; reset password; sign-in logs
- Google Workspace Admin — search users; suspend/restore; reset password; manage groups
- Auth0 — list users; reset password; rotate client secret; view logs
- Duo — list devices; reset auth; bypass; policy status

## Secrets & Password Managers
- HashiCorp Vault — read secret; write secret (guarded); list paths; token lookup
- 1Password — get item; search items; create item (guarded); share link
- Bitwarden — get item; search vault; create item (guarded); collections

## Data Warehouses & Queries
- Snowflake — run query; query history; resume/suspend warehouse; export results
- BigQuery — run query; list datasets; export table; job status
- Redshift — run query; query stats; pause/resume cluster; export
- Databricks — run SQL; run job; job status; fetch output
- PostgreSQL (read-only) — run query; explain; list tables; export CSV

## ETL / Reverse ETL
- Fivetran — list connectors; trigger sync; status; pause/resume
- Airbyte — list connections; trigger sync; logs; pause/resume
- dbt Cloud — trigger job; run status; fetch artifacts; failures
- Hightouch — trigger sync; status; pause; error report
- Census — trigger sync; status; pause; error report

## BI & Reporting
- Looker — run query; export dashboard; schedule delivery; share link
- Tableau — refresh extract; export view; schedule; workbook metadata
- Power BI — refresh dataset; export report; list dashboards; usage metrics
- Metabase — run query; export results; schedule pulse; list questions

## CRM & Sales
- Salesforce — search leads/contacts/accounts; create/update; log activity; update opportunity stage
- HubSpot CRM — search contacts/companies/deals; create/update; log note; enroll in sequence
- Pipedrive — search deals; create/update; move stage; add note/activity
- Dynamics 365 — search accounts/contacts; create/update; add activity; update stage
- Outreach — enroll prospect; pause/resume sequence; log reply; update stage
- Salesloft — add person; start cadence; log call/email; update stage

## Customer Support
- Zendesk Support — search tickets; get ticket; reply; internal note; change status/assignee
- Intercom — search conversations; reply; assign; tag; create ticket
- Freshdesk — search tickets; reply/note; assign; change status; merge
- Help Scout — search; reply; assign; tag; change status
- Front — search inbox; reply; assign; tag; move conversation
- Gorgias — search tickets; reply; assign; tag; change status

## Customer Messaging & Notifications
- Twilio — send SMS/WhatsApp; fetch delivery status; inbound handling; number lookup
- SendGrid — send email; manage templates; suppressions; delivery stats
- Postmark — send email; templates; bounce status; delivery stats
- Mailgun — send email; events; suppressions; routes
- Firebase Cloud Messaging — send push; send to topic; basic delivery status

## Marketing Automation
- Mailchimp — create campaign draft; add member; schedule send; campaign report
- Klaviyo — update profile; trigger flow; campaign draft; metrics
- Braze — trigger campaign/canvas; update user attributes; segment; metrics
- Customer.io — trigger campaign; update profile; segment; message history
- Marketo — sync lead; trigger campaign; activity log; program status

## Product Analytics & Feature Flags
- Amplitude — run chart; fetch cohorts; annotation; export
- Mixpanel — run report; funnels; cohort; export
- PostHog — insights query; manage feature flags; annotate
- LaunchDarkly — list flags; toggle; set rollout; audit events
- Statsig — list experiments; update rollout; fetch results; audit

## Feedback & Surveys
- Canny — create request; vote; comment; change status; merge duplicates
- Productboard — create insight; link to feature; update status; notes
- Typeform — list forms; fetch responses; export CSV; webhooks
- SurveyMonkey — list surveys; fetch responses; export; summary stats
- Delighted — fetch NPS responses; send survey; export; tag feedback

## Finance & Accounting
- QuickBooks Online — create invoice; record payment; list customers; export reports
- Xero — create invoice; approve; record payment; list contacts; reports
- NetSuite — search transactions; create/update record; approve PO; export report
- Bill.com — create bill; approve; schedule payment; vendor lookup
- Expensify — submit expense; approve; reimbursement status; policy lookup
- Ramp — list transactions; approve expense; policy check; export
- Brex — list transactions; create expense; manage cards; export

## Payments
- Stripe — search customer; create checkout/payment link; refund; dispute status; invoices
- PayPal — invoice; capture; refund; dispute status
- Square — invoice; take payment; refund; catalog lookup
- Adyen — payment status; refund; disputes; payout reports

## E-commerce
- Shopify — list orders; fulfill/cancel; update tracking; manage customers; discounts
- WooCommerce — list orders; update status; refund; manage products
- BigCommerce — list orders; update fulfillment; manage products; refunds
- Magento/Adobe Commerce — list orders; update status; manage products; refunds

## Shipping & Logistics
- Shippo — create label; track shipment; void label; rate quote
- ShipStation — list orders; create label; update shipment; tracking
- EasyPost — create shipment; buy label; track; void/refund
- AfterShip — track; webhook events; customer notifications; analytics

## Legal & Signatures
- DocuSign — create envelope; send; status; download signed PDF
- Adobe Sign — create agreement; send; status; download
- Dropbox Sign — create request; send; status; download
- Ironclad — create contract; route approval; status; search clauses
- PandaDoc — create doc; send; status; download

## HR & Recruiting
- Greenhouse — list jobs; search candidates; add note; move stage; schedule interview
- Lever — search candidates; add note; move stage; schedule; offer status
- Ashby — search candidates; update stage; add note; schedule
- BambooHR — directory lookup; time-off requests; org chart; announcements
- Rippling — onboard/offboard checklist; app access requests; device status

## Publishing & CMS
- WordPress — create draft; update/publish; upload media; moderate comments
- Ghost — create draft; publish; tags; newsletter send
- Webflow — publish site; update CMS item; upload asset; form submissions
- Contentful — create/update entry; publish; upload asset; query entries
- Sanity — create/update document; publish; query; upload asset

## Social & Community
- X (Twitter) — post; schedule; fetch mentions; reply; delete
- LinkedIn — create post; fetch comments; reply; company page posting
- YouTube — upload; fetch comments; reply; basic analytics
- Reddit — submit post; fetch comments; reply; mod actions (if authorized)
- Discourse — create topic; reply; moderate; tags/categories

## Design & Whiteboards
- Figma — get file metadata; export frames; list components/styles; comment
- Miro — list boards; create sticky; update sticky; export board
- Mural — list murals; create item; comment; export

## Automation Platforms & Webhooks
- Zapier — trigger zap; task history; list zaps; connection status
- Make (Integromat) — run scenario; list scenarios; execution logs; schedule toggle
- n8n — trigger workflow; list workflows; execution status; logs
- Workato — run recipe; job status; logs; connections
- Generic Webhook — POST payload; signed request; retry; validate response
