# Codex Cloud Threat Model

## Assets
- Source repositories, execution artifacts, and generated code hosted within Codex Cloud.
- Authentication secrets and tokens enabling access to the control plane and CLI integration.
- Supervisor-managed execution environments and their runtime credentials.

## Trust Boundaries
1. **Public Internet ↔ Edge** – Traffic from browsers and CLI clients terminates at the CDN / API gateway.
2. **Edge ↔ Application Tier** – Requests forwarded into frontend and API containers via private networking.
3. **Application Tier ↔ Data Stores** – API service communicates with the database, artifact store, and task queue using service credentials.
4. **Application Tier ↔ Supervisors** – Supervisors authenticate with API-issued tokens when polling for jobs and uploading results.

## Threat Scenarios
| ID | Scenario | Risk | Mitigations |
| --- | --- | --- | --- |
| T1 | Credential stuffing or brute-force login attempts against the API. | Medium | Rate limiting, credential lockout, MFA requirements for privileged users, audit logging. |
| T2 | Compromise of supervisor token leading to malicious task execution. | High | Short-lived tokens scoped per environment, mTLS between supervisors and API, rotation automation, anomaly detection. |
| T3 | SQL injection or query manipulation via API inputs. | High | Use parameterized queries via ORM, enforce request validation with typed schemas, fuzz testing of input handlers. |
| T4 | Artifact tampering or exfiltration. | Medium | Store artifacts in write-once buckets, sign artifact metadata, require presigned URLs with expiration. |
| T5 | Lateral movement from compromised frontend container. | Medium | Apply strict network policies, run containers as non-root, and enforce minimal filesystem permissions. |
| T6 | Denial of Service through unbounded task submissions. | Medium | Implement quota enforcement, rate limiting, and autoscaling policies with circuit breakers. |

## Security Controls
- Enforce OIDC integration for human users and service accounts, delegating authentication to a central IdP.
- Apply Infrastructure-as-Code (Terraform) to manage secrets, IAM policies, and network boundaries with code review.
- Adopt continuous vulnerability scanning for container images and base dependencies.
- Instrument structured audit logging for create/update/delete operations across API resources.
- Require SLSA provenance on supervisor container builds and verify signatures at deploy time.

## Residual Risks
- Temporary exposure of metadata if API reverse proxy misconfiguration occurs; mitigated through automated config linting.
- Operational complexity of token rotation for distributed supervisors; requires playbooks and automation.
- Dependency on third-party artifact storage uptime; plan for regional replication and documented recovery RTO/RPO.
