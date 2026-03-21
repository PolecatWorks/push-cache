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
    private final CacheFactory cacheFactory;
    private final Environment environment;
    private final Function<Properties, Consumer<String, byte[]>> consumerFactory;
    private final SchemaService schemaService;
    private final MetricsService metricsService;
    private final LagClearedHealthIndicator lagHealthIndicator;
    private final AtomicBoolean running = new AtomicBoolean(false);
    private long lastLagCheck = 0;
    private static final long LAG_CHECK_INTERVAL = 1000;

    @Autowired
    public KafkaConsumerService(AppConfig appConfig, CacheFactory cacheFactory, Environment environment,
            SchemaService schemaService, MetricsService metricsService, LagClearedHealthIndicator lagHealthIndicator) {
        this(appConfig, cacheFactory, environment, schemaService, metricsService, lagHealthIndicator, KafkaConsumer::new);
    }

    public KafkaConsumerService(AppConfig appConfig, CacheFactory cacheFactory, Environment environment,
            SchemaService schemaService, MetricsService metricsService, LagClearedHealthIndicator lagHealthIndicator,
            Function<Properties, Consumer<String, byte[]>> consumerFactory) {
        this.appConfig = appConfig;
        this.cacheFactory = cacheFactory;
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
            Properties props = getConsumerProperties();
            props.put(ConsumerConfig.ENABLE_AUTO_COMMIT_CONFIG, "true");
            props.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG,
                    appConfig.getKafka().getOffsetReset().toString().toLowerCase());

            try (Consumer<String, byte[]> consumer = consumerFactory.apply(props)) {
                consumer.subscribe(Collections.singletonList(appConfig.getKafka().getTopic()),
                        new ConsumerRebalanceListener() {
                            @Override
                            public void onPartitionsRevoked(Collection<TopicPartition> partitions) {
                                logger.info("Partitions revoked: {}", partitions);
                            }

                            @Override
                            public void onPartitionsAssigned(Collection<TopicPartition> partitions) {
                                logger.info("Partitions assigned: {}", partitions);
                                if (appConfig.getKafka().isForceResetEarliest()) {
                                    logger.info("Seeking to beginning for partitions: {}", partitions);
                                    consumer.seekToBeginning(partitions);
                                }
                            }
                        });
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

    private Properties getConsumerProperties() {
        Properties props = new Properties();
        String host = appConfig.getKafka().getBrokers().getHost();
        int port = appConfig.getKafka().getBrokers().getPort();
        if (host == null || port == -1) {
            throw new RuntimeException(
                    "Kafka broker host or port not defined in URI: " + appConfig.getKafka().getBrokers());
        }
        String brokerString = host + ":" + port;

        String groupId = appConfig.getKafka().getGroupId();
        if (appConfig.getKafka().isUseHostnameAsGroupId()) {
            groupId = System.getenv("HOSTNAME");
            if (groupId == null || groupId.isBlank()) {
                throw new RuntimeException("HOSTNAME environment variable is required when useHostnameAsGroupId is true");
            }
        }

        props.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, brokerString);
        props.put(ConsumerConfig.GROUP_ID_CONFIG, groupId);
        props.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, StringDeserializer.class);
        props.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class);
        return props;
    }

    private void checkLag(Consumer<String, byte[]> consumer) {
        try {
            Set<TopicPartition> assignment = consumer.assignment();
            if (assignment.isEmpty())
                return;

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
                cacheFactory.getAllStores().forEach(store -> store.remove(key).block());
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

        ByteBuffer buffer = ByteBuffer.wrap(value);
        buffer.get(); // Skip magic byte
        int schemaId = buffer.getInt();

        try {
            org.apache.avro.Schema schema = schemaService.getSchema(schemaId);
            String fullName = schema.getFullName();
            Cache store = cacheFactory.getStoreForSchema(fullName);

            if (store != null) {
                if (key != null) {
                    store.put(key, value).block();
                    metricsService.incrementUpdatesReceived();
                }
            } else {
                logger.warn("No store routed for schema {}", fullName);
                metricsService.incrementSchemaUnroutedCount();
            }
        } catch (Exception e) {
            logger.error("Failed to process message with schema ID {}", schemaId, e);
        }
    }
}
