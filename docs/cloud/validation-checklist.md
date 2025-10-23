# Codex Cloud Validation Checklist

This checklist documents the manual acceptance coverage required before promoting a Compose stack build to staging or production. Each scenario should be exercised end-to-end with the most recent container images and verified against both UI and API outputs.

## Prerequisites

- Compose stack (`cloud/docker-compose.yml`) is running with a clean SQLite volume.
- Seed data has been provisioned using `cloud/scripts/cli-contract.sh` or the Playwright fixtures.
- Test operator credentials:
  - Email: `codex-e2e@example.com`
  - Password: `codex-e2e`
- Confirm that background supervisor workers are idle to avoid mutating test tasks mid-run.

## Acceptance Scenarios

| Scenario | Detailed steps | Expected outcome | Owner |
| --- | --- | --- | --- |
| Authentication happy path | Launch frontend, submit login form with baseline credentials, verify token persistence after reload. | Redirect to `/tasks`, Ant Design success toast, `localStorage.codex-cloud-token` populated. | Cloud QA |
| Authentication guard | Visit `/tasks` without a token, verify redirect to login and banner message. | Guard enforces redirect, no API calls with missing token. | Cloud QA |
| Task list lifecycle | Filter by each status, refresh table, and confirm newest task appears from fixture provisioning. | Table renders five status tags with translated labels, pagination works, API returns HTTP 200. | Runtime |
| Task detail lifecycle | Claim pending task, start attempt, attach diff/log payload, and complete as succeeded. | Status transitions `pending → claimed → running → review`, attempt list updates with localized badges. | Runtime |
| Artifact retrieval | From a completed attempt, open diff/log modal and validate formatting plus download behaviour. | Modal loads artifact text, handles network failure gracefully. | Runtime |
| CLI contract | Execute `cloud/scripts/cli-contract.sh` against the stack and validate artifacts + attempt creation. | CLI exits 0, attempt recorded under automation environment with captured artifacts. | Tooling |
| Supervisor reconciliation | Start supervisor container, observe attempt pickup, and verify completion webhook results. | Supervisor logs show reconciliation loop with zero errors. | Platform |

## Dry Run Schedule

| Window | Cadence | Scope | Participants |
| --- | --- | --- | --- |
| Tuesday 15:00–16:00 UTC | Weekly | Authentication + task lifecycle | Cloud QA, Runtime |
| Wednesday 09:00–09:30 UTC | Bi-weekly | CLI contract + supervisor reconciliation | Tooling, Platform |
| Friday 17:00–17:30 UTC | Release week only | Full regression, including artifact review | Cloud QA, Runtime, Tooling |

Document the execution status in the release issue tracker after each window. Deviations, bugs, or skipped steps must be filed as GitHub issues with links to Playwright traces or CLI artifacts for traceability.
