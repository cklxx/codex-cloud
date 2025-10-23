# Codex Cloud Architecture Diagram

```mermaid
graph TD
    subgraph Client Tier
        Browser["Web Browser"]
        CLITool["Codex CLI"]
    end

    subgraph Edge
        CDN[(CDN / TLS Termination)]
    end

    subgraph Application Tier
        FE["Next.js Frontend"]
        API["Rust API (Axum)"]
        Supervisor["Supervisor Workers"]
    end

    subgraph Data & Storage
        DB[(Primary Database)]
        Artifacts[(Artifact Storage)]
        Queue[(Task Queue)]
    end

    Browser -->|HTTPS| CDN --> FE
    CLITool -->|HTTPS| API
    FE -->|REST / WebSocket| API
    API -->|SQL| DB
    API -->|Signed URLs| Artifacts
    API -->|Enqueue Tasks| Queue
    Supervisor -->|Poll Jobs| API
    Supervisor -->|Fetch Artifacts| Artifacts
    Supervisor -->|Update Status| API
```

## Component Responsibilities

- **Next.js Frontend** – Serves the operator dashboard, authenticates via the API, and streams task updates to users.
- **Rust API** – Implements authentication, repository onboarding, task execution workflows, and artifact publishing.
- **Supervisor Workers** – Execute queued tasks against managed environments while reporting status and artifacts back to the API.
- **Primary Database** – Stores identities, repositories, environments, task metadata, and audit trails.
- **Artifact Storage** – Houses generated artifacts and execution logs for download.
- **Task Queue** – Buffers execution requests for asynchronous supervisor processing.

## Deployment Notes

- Containers are orchestrated via Docker Compose for local development, with a path to ECS/Kubernetes by mirroring the service boundaries above.
- Secrets are injected with environment variables and should be sourced from a managed secrets store in production.
- Each component exposes structured health checks for observability and workload autoscaling.
