# Codex Cloud Performance Baseline

Baseline measurements were collected on 2024-10-20 using the Compose stack running on an 8 vCPU / 16 GiB Ubuntu 22.04 host. The API container was warmed up for five minutes before sampling. Authentication credentials followed the E2E defaults documented in the validation checklist.

## Hyperfine (single-request latency)

Command: `OUTPUT_DIR=artifacts/perf RUNS=20 scripts/perf/hyperfine.sh`

| Endpoint | Mean (ms) | StdDev (ms) | p95 (ms) |
| --- | --- | --- | --- |
| `GET /health` | 18.6 | 2.9 | 23.4 |
| `GET /tasks` | 72.1 | 8.7 | 89.5 |
| `GET /api/codex/environments` | 64.3 | 7.5 | 83.8 |

Notes:
- `/tasks` and `/api/codex/environments` include authorization headers; cache priming reduced p95 by ~12% versus cold runs.
- The SQLite volume resided on the host filesystem; moving to tmpfs improved `/tasks` mean latency to 61 ms during ad-hoc experiments.

## K6 (steady load)

Command: `k6 run -e API_BASE=http://127.0.0.1:8000 -e ACCESS_TOKEN=$TOKEN scripts/perf/k6.js`

| Metric | Result |
| --- | --- |
| Requests per second | 95.7 |
| HTTP `p(95)` duration | 1.11 s |
| `tasks_list_duration` average | 612 ms |
| `environments_duration` average | 577 ms |
| Error rate | 0% |

Observations:
- CPU utilization on the API container peaked at 72%, leaving headroom for burst traffic.
- No allocator warnings were observed in the API logs; memory plateaued at ~420 MiB.
- Increasing the arrival rate to 150 req/s triggered queueing with `p(95)` at 1.9 s; further tuning should include indexing on `tasks.updated_at`.

## Next Steps

1. Automate daily hyperfine runs and append JSON exports to the artifact bucket for regression tracking.
2. Extend the K6 script with a task creation step to capture write-path latency.
3. Evaluate connection pooling limits in the SQLite driver before scaling beyond ~200 concurrent VUs.
