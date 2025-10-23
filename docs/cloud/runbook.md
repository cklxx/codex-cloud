# Codex Cloud Operations Runbook

This runbook covers the day-two operations for Codex Cloud. Use it as the first
stop during incidents.

## Incident Response Checklist

1. **Acknowledge the alert.** Silence duplicates in Grafana Alerting or
   Prometheus Alertmanager as needed.
2. **Assess user impact.** Determine whether the Codex API, frontend, or
   supervisor is degraded. Confirm via health checks:
   - API: `curl -f https://codex.example.com/healthz`
   - Frontend: `curl -I https://codex.example.com`
   - Supervisor: `docker compose logs supervisor`
3. **Stabilize the platform.**
   - Restart the compose stack if the majority of services are unhealthy:
     ```sh
     sudo systemctl restart codex-compose.service
     ```
   - Restart only the failing container if scope is limited:
     ```sh
     cd /opt/codex/cloud
     sudo docker compose -f docker-compose.production.yml restart <service>
     ```
4. **Investigate.** Collect logs (see below) and recent deployments or
   infrastructure changes. Capture timelines in your incident doc.
5. **Communicate.** Update the status page and inform stakeholders. Provide ETA
   or mitigations.
6. **Post-incident.** File a ticket for follow-up actions, including restoring
   capacity, tuning alerts, or improving docs.

## Log Locations

| Component   | Location | Notes |
| ----------- | -------- | ----- |
| Docker Compose | `sudo journalctl -u codex-compose.service` | Stack lifecycle and failures |
| API | `sudo docker logs api` | Structured request metrics at `/var/lib/docker/containers` |
| Frontend | `sudo docker logs frontend` | Reverse proxy errors, SSR output |
| Supervisor | `sudo docker logs supervisor` | Firecracker lifecycle, VM metrics |
| Ignite Supervisor | `sudo journalctl -u codex-ignite-supervisor.service` | Host-level Firecracker events |
| Host metrics | `/var/log/prometheus/` and `/var/log/node-exporter/` | Exporter diagnostic logs |

When debugging Firecracker VMs, also inspect `/var/lib/ignite` for instance
artifacts and `/var/log/ignite` for crash details.

## Snapshot and Backup Procedures

Backups run via `cloud/ops/backup/backup.sh`. Trigger an ad-hoc snapshot with:

```sh
sudo BACKUP_ROOT=/var/backups/codex cloud/ops/backup/backup.sh
```

### Regenerating Snapshots

1. Stop API writes if possible (maintenance mode or supervisor drain).
2. Run the backup script manually (above) and verify the output directory.
3. Confirm that `manifest.json` lists the expected database and artifact paths.
4. Upload the snapshot to off-site storage (S3 bucket `codex-backups`).

### Restoring From Snapshots

1. Identify the timestamped backup folder in `/var/backups/codex` or fetch it
   from off-site storage.
2. Restore using the helper script (defaults to the latest snapshot):
   ```sh
   sudo BACKUP_ROOT=/var/backups/codex cloud/ops/backup/restore.sh
   ```
   To target a specific snapshot:
   ```sh
   sudo BACKUP_ROOT=/var/backups/codex cloud/ops/backup/restore.sh 20240101T000000Z
   ```
3. Validate service health:
   - `sudo systemctl status codex-compose.service`
   - `curl -f https://codex.example.com/healthz`
4. Re-enable writes and notify stakeholders that the restore is complete.

## Monitoring

- Prometheus: https://codex-monitoring.example.com/prometheus
- Grafana: https://codex-monitoring.example.com/grafana (admin credentials stored
  in the secret manager `codex/prod/grafana`).

Dashboards:
- **Codex Cloud Overview** – request rate, Firecracker exits, host load.
- **Infra Health** – build custom dashboards under `cloud/ops/monitoring/grafana-dashboards/`.

## Contact Ladder

1. On-call engineer (rotate weekly).
2. Platform team lead.
3. Head of Engineering.

Escalate to vendor support (cloud provider, Ignite maintainers) when internal
triage is exhausted or the incident threatens SLAs.
