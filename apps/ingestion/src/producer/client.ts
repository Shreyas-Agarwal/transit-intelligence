import { Kafka, type Producer, type Admin, CompressionTypes } from 'kafkajs';
import { Logger } from '@transit-intelligence/shared-logger';
import { config } from '../config.js';
import { TOPICS, TOPIC_CONFIGS, type TopicName } from './topics.js';

const logger = new Logger('Redpanda-Client');

/**
 * Creates and returns a configured KafkaJS Kafka instance pointing at Redpanda.
 *
 * KafkaJS is fully compatible with Redpanda's Kafka API.
 * No Redpanda-specific client libraries are required for basic produce/consume.
 */
export function createKafkaClient(): Kafka {
  return new Kafka({
    clientId: config.kafkaClientId,
    brokers: config.redpandaBrokers,
    // Retry configuration — reasonable defaults for local exploration
    retry: {
      initialRetryTime: 300,
      retries: 5,
    },
  });
}

/**
 * Creates a KafkaJS producer with default settings.
 * The caller is responsible for calling producer.connect() and producer.disconnect().
 */
export function createProducer(kafka: Kafka): Producer {
  return kafka.producer({
    // Wait for the leader to confirm the write — safe default for exploration
    allowAutoTopicCreation: false,
  });
}

/**
 * Creates a KafkaJS admin client.
 * Used at startup to ensure required topics exist in Redpanda.
 */
export function createAdminClient(kafka: Kafka): Admin {
  return kafka.admin();
}

/**
 * Ensures all ADR 0008 topics exist in Redpanda, creating them if missing.
 *
 * This is idempotent — safe to call on every startup.
 * Existing topics are left unchanged.
 *
 * @param kafka KafkaJS Kafka instance.
 */
export async function ensureTopics(kafka: Kafka): Promise<void> {
  const admin = createAdminClient(kafka);
  await admin.connect();

  try {
    const existingTopics = await admin.listTopics();
    const topicsToCreate = (Object.values(TOPICS) as TopicName[]).filter(
      (topic) => !existingTopics.includes(topic),
    );

    if (topicsToCreate.length === 0) {
      logger.info('All Redpanda topics already exist — no creation needed', {
        topics: Object.values(TOPICS),
      });
      return;
    }

    logger.info('Creating missing Redpanda topics', { topics: topicsToCreate });

    await admin.createTopics({
      topics: topicsToCreate.map((topicName) => {
        const cfg = TOPIC_CONFIGS[topicName];
        return {
          topic: topicName,
          numPartitions: cfg.numPartitions,
          replicationFactor: cfg.replicationFactor,
          configEntries: [{ name: 'retention.ms', value: String(cfg.retentionMs) }],
        };
      }),
    });

    logger.info('Topics created successfully', { topics: topicsToCreate });
  } finally {
    await admin.disconnect();
  }
}

export { CompressionTypes };
