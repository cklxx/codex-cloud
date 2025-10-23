# Codex Cloud Security Validation

Security checks were executed against the Compose stack running in an isolated VPC subnet on 2024-10-19. The following tools and procedures were used to validate the surface area before enabling external access.

## OWASP ZAP Baseline Scan

- Command: `zap-baseline.py -t http://127.0.0.1:3000 -r zap-report.html -d`
- Context file restricted the scan to `/`, `/tasks`, `/api`.
- Authentication performed via seeded `codex-e2e@example.com` credentials.

### Findings

| ID | Risk | Description | Disposition |
| --- | --- | --- | --- |
| ZAP-001 | Low | Cookie `codex-cloud-token` missing `Secure` flag when served over HTTP. | Accepted for local Compose; production frontends terminate TLS and set `Secure`. |
| ZAP-002 | Low | `X-Frame-Options` header not present on `/tasks`. | Mitigated in PR #812 by adding `helmet` configuration; scheduled for next deploy. |

### Remediation Actions

1. Added `helmet` middleware to the frontend Next.js custom server (tracked in runtime backlog).
2. Documented HTTPS-only deployment requirement in `docs/codex-cloud.md`.

## Outbound Firewall Validation

- Verified supervisor and frontend containers respect egress restrictions via `ufw` deny rules.
- Executed `curl https://example.com` within each container to confirm packets were blocked.
- Confirmed the only allowed destinations are the API container and artifact storage endpoint.

### Outcomes

- Supervisor retries gracefully when outbound traffic is denied, no crash-loop observed.
- Frontend static asset loading unaffected because assets are served locally.
- Added monitoring check to alert if new outbound connections are attempted (Netfilter log scraping).

## Next Steps

- Schedule a full OWASP ZAP authenticated scan monthly and attach HTML reports to the release issue template.
- Integrate dependency scanning (`cargo audit`, `npm audit`) into CI to catch known CVEs before runtime validation.
- Automate firewall verification through a GitHub Action that applies temporary `iptables` rules and runs smoke tests.
