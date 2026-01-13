# Jira Service Management Integration for Operai Toolbox

Interact with Jira Service Management (formerly Jira Service Desk) through its REST API.

## Overview

This integration enables Operai Toolbox to interact with Jira Service Management service desks and requests.

- **Search and retrieve** service requests with filtering capabilities
- **Add comments** to existing requests with public/private visibility control
- **Transition** requests through workflow statuses
- **Assign** requests to users by account ID

**Primary use cases:**
- Customer support workflows and ticket management
- IT service desk automation
- Integration with AI agents for request triage and response

## Authentication

This integration uses **Basic Authentication** with system credentials. Credentials are supplied via environment variables or the toolbox credential system.

### Required Credentials

- `username`: Your Atlassian account email address
- `password`: An API token (not your account password)
- `endpoint` (optional): Custom API endpoint URL (defaults to `https://your-domain.atlassian.net`)

### Obtaining an API Token

1. Visit https://id.atlassian.com/manage-profile/security/api-tokens
2. Click "Create API token"
3. Label the token (e.g., "Operai Toolbox")
4. Copy the generated token immediately

### Manifest Configuration

Add the following to your `operai.toml` manifest:

```toml
[[tools]]
package = "brwse-jira-service-management"
[tools.credentials.jira_service_management]
username = "your-email@example.com"
password = "your-api-token"
# endpoint = "https://your-domain.atlassian.net"  # optional
```

## Available Tools

### search_requests
**Tool Name:** Search Jira Service Management Requests
**Capabilities:** read
**Tags:** jira, service-management, tickets
**Description:** Search service requests in Jira Service Management

**Input:**
| Field | Type | Description |
|-------|------|-------------|
| `service_desk_id` | `string` | Service desk ID to search within |
| `query` | `string?` | JQL query to filter requests (optional) |
| `limit` | `number?` | Maximum number of results (1-100, defaults to 50) |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `requests` | `RequestSummary[]` | List of request summaries with issue ID, key, type, status, reporter, and creation date |

### get_ticket
**Tool Name:** Get Jira Service Management Ticket
**Capabilities:** read
**Tags:** jira, service-management, tickets
**Description:** Retrieve a single service request by ID or key

**Input:**
| Field | Type | Description |
|-------|------|-------------|
| `issue_id_or_key` | `string` | Issue key (e.g., "SD-123") or issue ID |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `request` | `RequestDetail` | Detailed request information including request type, status, reporter, and field values |

### comment
**Tool Name:** Add Comment to Jira Service Management Ticket
**Capabilities:** write
**Tags:** jira, service-management, tickets
**Description:** Add a comment to a service request

**Input:**
| Field | Type | Description |
|-------|------|-------------|
| `issue_id_or_key` | `string` | Issue key (e.g., "SD-123") or issue ID |
| `body` | `string` | Comment body text |
| `public` | `boolean?` | Whether the comment is visible to customers (defaults to true) |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `comment` | `Comment` | Created comment with ID, body, author, timestamp, and visibility |

### transition
**Tool Name:** Transition Jira Service Management Ticket
**Capabilities:** write
**Tags:** jira, service-management, tickets
**Description:** Transition a service request to a different status

**Input:**
| Field | Type | Description |
|-------|------|-------------|
| `issue_id_or_key` | `string` | Issue key (e.g., "SD-123") or issue ID |
| `transition_id` | `string` | Transition ID to perform |
| `comment` | `string?` | Optional comment to add with the transition |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `success` | `boolean` | Success status of the transition operation |

### assign
**Tool Name:** Assign Jira Service Management Ticket
**Capabilities:** write
**Tags:** jira, service-management, tickets
**Description:** Assign a service request to a user

**Input:**
| Field | Type | Description |
|-------|------|-------------|
| `issue_id_or_key` | `string` | Issue key (e.g., "SD-123") or issue ID |
| `account_id` | `string` | Account ID of the user to assign |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `success` | `boolean` | Success status of the assignment operation |

## API Documentation

- **Base URL:** `https://your-domain.atlassian.net` (configurable via `endpoint` credential)
- **API Documentation:** https://developer.atlassian.com/cloud/jira/service-desk/rest/

**Implementation notes:**
- Most operations use the Service Desk API (`/rest/servicedeskapi/`)
- Assignment uses the Jira Platform API (`/rest/api/3/`)
- All requests require Basic Authentication with username and API token
- Self-hosted instances can specify a custom endpoint URL

## Testing

Run tests for this integration:

```bash
cargo test -p brwse-jira-service-management
```

Tests include:
- Input validation for all tools
- Serialization roundtrips for data types
- Mock server integration tests for successful operations
- Error handling tests for failed requests

## Development

- **Crate:** `brwse-jira-service-management`
- **Source:** `examples/project-tasks/jira-service-management/`
