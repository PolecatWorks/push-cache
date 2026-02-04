package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.AppConfig;
import org.apache.kafka.clients.consumer.*;
import org.apache.kafka.common.TopicPartition;
import org.apache.kafka.common.serialization.ByteArrayDeserializer;
import org.apache.kafka.common.serialization.StringDeserializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.core.env.Environment;
import org.springframework.stereotype.Service;

import java.nio.ByteBuffer;
import java.time.Duration;
import java.util.*;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Function;

@Service
public class KafkaConsumerService {

    private static final Logger logger = LoggerFactory.getLogger(KafkaConsumerService.class);
    private final AppConfig appConfig;
    private final CacheStore cacheStore;
    private final Environment environment;
    private final Function<Properties, Consumer<String, byte[]>> consumerFactory;
    private final SchemaService schemaService;
    private final MetricsService metricsService;
    private final LagClearedHealthIndicator lagHealthIndicator;
    private final AtomicBoolean running = new AtomicBoolean(false);
    private long lastLagCheck = 0;
    private static final long LAG_CHECK_INTERVAL = 1000;

    @Autowired
    public KafkaConsumerService(AppConfig appConfig, CacheStore cacheStore, Environment environment, SchemaService schemaService, MetricsService metricsService, LagClearedHealthIndicator lagHealthIndicator) {
        this(appConfig, cacheStore, environment, schemaService, metricsService, lagHealthIndicator, KafkaConsumer::new);
    }

    public KafkaConsumerService(AppConfig appConfig, CacheStore cacheStore, Environment environment, SchemaService schemaService, MetricsService metricsService, LagClearedHealthIndicator lagHealthIndicator, Function<Properties, Consumer<String, byte[]>> consumerFactory) {
        this.appConfig = appConfig;
        this.cacheStore = cacheStore;
        this.environment = environment;
        this.schemaService = schemaService;
        this.metricsService = metricsService;
        this.lagHealthIndicator = lagHealthIndicator;
        this.consumerFactory = consumerFactory;
    }

    public void start() {
        String webType = environment.getProperty("spring.main.web-application-type");
        if ("none".equalsIgnoreCase(webType)) {
            logger.info("Web application type is 'none'. Skipping Kafka consumer start.");
            return;
        }

        if (running.compareAndSet(false, true)) {
            Thread consumerThread = new Thread(this::runConsumer, "kafka-consumer-thread");
            consumerThread.start();
        }
    }

    public void stop() {
        running.set(false);
    }

    private void runConsumer() {
        try {
            if (appConfig.getKafka().isForceResetEarliest()) {
                resetOffsets();
            }

            Properties props = getConsumerProperties();
            props.put(ConsumerConfig.ENABLE_AUTO_COMMIT_CONFIG, "true");
            props.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG, appConfig.getKafka().getOffsetReset().toString().toLowerCase());

            try (Consumer<String, byte[]> consumer = consumerFactory.apply(props)) {
                consumer.subscribe(Collections.singletonList(appConfig.getKafka().getTopic()));
                logger.info("Started Kafka consumer for topic: {}", appConfig.getKafka().getTopic());

                while (running.get()) {
                    ConsumerRecords<String, byte[]> records = consumer.poll(Duration.ofMillis(100));
                    for (ConsumerRecord<String, byte[]> record : records) {
                        processRecord(record);
                    }

                    long now = System.currentTimeMillis();
                    if (now - lastLagCheck > LAG_CHECK_INTERVAL) {
                        checkLag(consumer);
                        lastLagCheck = now;
                    }
                }
            }
        } catch (Exception e) {
            logger.error("Error in Kafka consumer loop", e);
            running.set(false);
        }
    }

    private void resetOffsets() {
        logger.info("Forcing consumer group offsets to earliest for topic: {}", appConfig.getKafka().getTopic());
        Properties props = getConsumerProperties();
        props.put(ConsumerConfig.ENABLE_AUTO_COMMIT_CONFIG, "false");

        try (Consumer<String, byte[]> consumer = consumerFactory.apply(props)) {
            String topic = appConfig.getKafka().getTopic();
            List<TopicPartition> partitions = new ArrayList<>();
            consumer.partitionsFor(topic).forEach(p -> partitions.add(new TopicPartition(topic, p.partition())));

            if (partitions.isEmpty()) {
                logger.warn("Topic {} has no partitions", topic);
                return;
            }

            consumer.assign(partitions);
            consumer.seekToBeginning(partitions);
            consumer.commitSync();
            logger.info("Successfully reset consumer group offsets to earliest.");
        } catch (Exception e) {
             logger.error("Failed to reset offsets", e);
             throw new RuntimeException("Failed to reset offsets", e);
        }
    }

    private Properties getConsumerProperties() {
        Properties props = new Properties();
        String host = appConfig.getKafka().getBrokers().getHost();
        int port = appConfig.getKafka().getBrokers().getPort();
        if (host == null || port == -1) {
             throw new RuntimeException("Kafka broker host or port not defined in URI: " + appConfig.getKafka().getBrokers());
        }
        String brokerString = host + ":" + port;

        props.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, brokerString);
        props.put(ConsumerConfig.GROUP_ID_CONFIG, appConfig.getKafka().getGroupId());
        props.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, StringDeserializer.class);
        props.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class);
        return props;
    }

    private void checkLag(Consumer<String, byte[]> consumer) {
        try {
            Set<TopicPartition> assignment = consumer.assignment();
            if (assignment.isEmpty()) return;

            Map<TopicPartition, Long> endOffsets = consumer.endOffsets(assignment);
            long totalLag = 0;

            for (TopicPartition tp : assignment) {
                long position = consumer.position(tp);
                Long end = endOffsets.get(tp);
                if (end != null) {
                    totalLag += Math.max(0, end - position);
                }
            }

            metricsService.setConsumerLag(totalLag);

            if (totalLag == 0 && !lagHealthIndicator.isCleared()) {
                logger.info("Startup lag cleared, application is now Ready");
                lagHealthIndicator.setCleared(true);
            }
        } catch (Exception e) {
            logger.warn("Failed to check consumer lag", e);
        }
    }

    private void processRecord(ConsumerRecord<String, byte[]> record) {
        String key = record.key();
        byte[] value = record.value();

        if (value == null) {
            // Tombstone
            if (key != null) {
                cacheStore.remove(key);
                metricsService.incrementTombstonesProcessed();
                logger.debug("Removed record for key: {}", key);
            }
            return;
        }

        // Avro check: Magic byte must be 0
        if (value.length < 5 || value[0] != 0x00) {
            logger.warn("Received invalid or non-confluent message for key: {}", key);
            metricsService.incrementSchemaMismatchCount();
            return;
        }

        metricsService.incrementUpdatesReceived();

        ByteBuffer buffer = ByteBuffer.wrap(value);
        buffer.get(); // Skip magic byte
        int schemaId = buffer.getInt();

        // Ensure schema is cached (prefetch)
        try {
            schemaService.getSchema(schemaId);
        } catch (Exception e) {
            logger.error("Failed to prefetch schema {}", schemaId, e);
            // We continue even if schema fetch failed?
            // If we can't get schema, the web handler will also fail.
            // But we still cache the bytes.
        }

        if (key != null) {
            cacheStore.put(key, value);
        }
    }
}
