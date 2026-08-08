//! Redpanda producer built on `rskafka` — a pure-Rust Kafka protocol client,
//! chosen over `rdkafka` so the workspace doesn't need a `librdkafka`/`cmake`
//! toolchain to build.
//!
//! Every Phase 1 topic (see [`crate::topics`]) has exactly one partition, so
//! a single [`rskafka::client::partition::PartitionClient`] per topic is
//! sufficient — there is no partitioning decision to make yet (ADR 0008
//! explicitly defers that until replay patterns are validated).

use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result};
use chrono::Utc;
use rskafka::client::partition::{Compression, PartitionClient, UnknownTopicHandling};
use rskafka::client::{Client, ClientBuilder};
use rskafka::record::Record;

use crate::model::KafkaMessage;
use crate::topics::TOPIC_CONFIGS;

/// Number of records published to a partition per `produce()` call.
const PRODUCE_BATCH_SIZE: usize = 100;

pub struct RedpandaProducer {
    client: Client,
    partition_clients: HashMap<&'static str, PartitionClient>,
}

impl RedpandaProducer {
    pub async fn connect(brokers: Vec<String>, client_id: String) -> Result<Self> {
        let client = ClientBuilder::new(brokers)
            .client_id(client_id)
            .build()
            .await
            .context("failed to connect to Redpanda brokers")?;

        Ok(Self {
            client,
            partition_clients: HashMap::new(),
        })
    }

    /// Ensures every Phase 1 topic exists, creating whichever are missing.
    ///
    /// Idempotent — safe to call on every startup. Note: this only sets
    /// partition count and replication factor at creation time; `rskafka`'s
    /// `create_topic` doesn't accept broker-side config entries, so retention
    /// (`retention.ms`) must still be set via `rpk topic alter-config` per
    /// `docs/runbooks/local-redpanda-setup.md` — this mirrors what that
    /// runbook already documents as the manual fallback.
    pub async fn ensure_topics(&mut self) -> Result<()> {
        let existing: Vec<String> = self
            .client
            .list_topics()
            .await
            .context("failed to list Redpanda topics")?
            .into_iter()
            .map(|t| t.name)
            .collect();

        for cfg in TOPIC_CONFIGS {
            if existing.iter().any(|name| name == cfg.name) {
                tracing::debug!(topic = cfg.name, "topic already exists");
                continue;
            }

            tracing::info!(topic = cfg.name, "creating missing Redpanda topic");
            self.client
                .controller_client()
                .context("failed to build Redpanda controller client")?
                .create_topic(cfg.name, cfg.num_partitions, cfg.replication_factor, 5_000)
                .await
                .with_context(|| format!("failed to create topic {}", cfg.name))?;
        }

        Ok(())
    }

    async fn partition_client(&mut self, topic: &'static str) -> Result<&PartitionClient> {
        if !self.partition_clients.contains_key(topic) {
            let client = self
                .client
                .partition_client(topic, 0, UnknownTopicHandling::Retry)
                .await
                .with_context(|| format!("failed to build partition client for {topic}"))?;
            self.partition_clients.insert(topic, client);
        }
        Ok(self.partition_clients.get(topic).expect("just inserted"))
    }

    /// Publishes `messages` to `topic` in batches of [`PRODUCE_BATCH_SIZE`].
    pub async fn publish(&mut self, topic: &'static str, messages: Vec<KafkaMessage>) -> Result<()> {
        let partition_client = self.partition_client(topic).await?;

        for batch in messages.chunks(PRODUCE_BATCH_SIZE) {
            let records = batch
                .iter()
                .map(|m| Record {
                    key: Some(m.key.clone().into_bytes()),
                    value: Some(m.value.clone().into_bytes()),
                    headers: BTreeMap::default(),
                    timestamp: Utc::now(),
                })
                .collect();

            partition_client
                .produce(records, Compression::default())
                .await
                .with_context(|| format!("failed to publish batch to {topic}"))?;
        }

        Ok(())
    }
}
