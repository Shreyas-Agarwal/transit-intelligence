# Local Redpanda Setup — Sprint 02

## Overview

During Sprint 02 (exploration phase), Redpanda runs natively inside WSL2 (Ubuntu). The production Docker Compose setup (`phase2` profile) is preserved for later.

From Windows, Redpanda is accessible at `localhost:9092` via WSL2's transparent port forwarding.

---

## Prerequisites

- Windows 11 / Windows 10 with WSL2 enabled
- Ubuntu distribution installed in WSL2

---

## Installation (WSL2/Ubuntu)

If Redpanda is not yet installed in your WSL2 environment:

```bash
# Add Redpanda apt repository
curl -1sLf 'https://dl.redpanda.com/nzc4ZYQK3WRGd9sy/redpanda/cfg/setup/bash.deb.sh' | sudo bash

# Install Redpanda
sudo apt-get install -y redpanda

# Configure as a development (single-node) instance
sudo rpk redpanda config bootstrap --self $(hostname -I | awk '{print $1}')
sudo rpk redpanda config set redpanda.empty_seed_starts_cluster true

# Start the broker
sudo systemctl start redpanda
```

> **Note**: If `systemctl` is unavailable in your WSL2 distro, start manually:
>
> ```bash
> sudo rpk redpanda start --overprovisioned --smp 1 &
> ```

---

## Verifying the Broker

```bash
# Confirm the broker is running and accessible
rpk cluster info

# Expected output:
# CLUSTER
# =======
# redpanda.cluster_id  ...
#
# BROKERS
# =======
# ID    HOST       PORT
# 0*    localhost  9092
```

---

## Topic Management

Topics are created automatically when the ingestion worker starts (`ensureTopics()` in `client.ts`). You can also manage them manually:

```bash
# List all topics
rpk topic list

# Create topics manually (Sprint 02 — 1 partition each)
rpk topic create transit.snapshots.raw         --partitions 1 --replicas 1
rpk topic create transit.snapshots.normalized  --partitions 1 --replicas 1
rpk topic create transit.state.deltas          --partitions 1 --replicas 1
rpk topic create transit.metrics.operational   --partitions 1 --replicas 1

# Inspect a topic
rpk topic describe transit.snapshots.raw

# Consume messages from the raw snapshot topic (human-readable JSON)
rpk topic consume transit.snapshots.raw

# Consume with offset (read from beginning)
rpk topic consume transit.snapshots.raw --offset start

# Consume only N messages
rpk topic consume transit.snapshots.raw --num 5
```

---

## Port Reference

| Port  | Service                       |
| ----- | ----------------------------- |
| 9092  | Kafka API (primary broker)    |
| 19092 | External Kafka API (optional) |
| 8082  | HTTP Proxy (PandaProxy)       |
| 9644  | Admin API                     |

---

## Connectivity from Windows (Node.js / TypeScript)

WSL2 automatically forwards `localhost` ports to the Windows host. The ingestion worker connects using:

```
REDPANDA_BROKERS=localhost:9092
```

No additional configuration is required. KafkaJS connects to `localhost:9092` on Windows and WSL2's NAT layer routes it to the Redpanda broker.

---

## Admin API Health Check

```bash
# From Windows PowerShell or WSL2:
curl http://localhost:9644/v1/brokers

# Expected response:
# [{"node_id":0,"num_cores":...,"membership_status":"active",...}]
```

---

## Stopping Redpanda

```bash
# If running via systemctl:
sudo systemctl stop redpanda

# If running in background:
sudo pkill -f redpanda
```

---

## Connecting to Docker Compose Redpanda (Phase 2)

When Phase 2 infrastructure is needed, start the full stack including Redpanda:

```bash
docker compose --profile phase2 up -d
```

The Docker Compose Redpanda service runs on the same ports as the WSL2 instance, so only one should be running at a time.
