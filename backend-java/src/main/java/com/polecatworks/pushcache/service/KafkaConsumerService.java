package com.polecatworks.pushcache.service;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.polecatworks.pushcache.config.AppConfig;
import org.apache.avro.Schema;
import org.apache.kafka.clients.consumer.*;
import org.apache.kafka.common.TopicPartition;
import org.apache.kafka.common.serialization.ByteArrayDeserializer;
import org.apache.kafka.common.serialization.StringDeserializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.core.env.Environment;
import org.springframework.stereotype.Service;
import org.springframework.web.client.RestClient;

import java.net.URI;
import java.nio.ByteBuffer;
import java.time.Duration;
import java.util.*;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Function;

@Service
public class KafkaConsumerService {

    private static final Logger logger = LoggerFactory.getLogger(KafkaConsumerService.class);
    private final AppConfig appConfig;
    private final CacheStore cacheStore;
    private final Environment environment;
    private final RestClient restClient;
    private final Function<Properties, Consumer<String, byte[]>> consumerFactory;
    private final Map<Integer, Schema> schemaCache = new ConcurrentHashMap<>();
    private final AtomicBoolean running = new AtomicBoolean(false);
    private final ObjectMapper objectMapper = new ObjectMapper();

    @Autowired
    public KafkaConsumerService(AppConfig appConfig, CacheStore cacheStore, Environment environment) {
        this(appConfig, cacheStore, environment, RestClient.builder().build(), KafkaConsumer::new);
    }

    public KafkaConsumerService(AppConfig appConfig, CacheStore cacheStore, Environment environment, RestClient restClient, Function<Properties, Consumer<String, byte[]>> consumerFactory) {
        this.appConfig = appConfig;
        this.cacheStore = cacheStore;
        this.environment = environment;
        this.restClient = restClient;
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

    private void processRecord(ConsumerRecord<String, byte[]> record) {
        String key = record.key();
        byte[] value = record.value();

        if (value == null) {
            // Tombstone
            if (key != null) {
                cacheStore.remove(key);
                logger.debug("Removed record for key: {}", key);
            }
            return;
        }

        // Avro check: Magic byte must be 0
        if (value.length < 5 || value[0] != 0x00) {
            logger.warn("Received invalid or non-confluent message for key: {}", key);
            return;
        }

        ByteBuffer buffer = ByteBuffer.wrap(value);
        buffer.get(); // Skip magic byte
        int schemaId = buffer.getInt();

        if (!schemaCache.containsKey(schemaId)) {
            fetchAndCacheSchema(schemaId);
        }

        if (key != null) {
            cacheStore.put(key, value);
        }
    }

    private void fetchAndCacheSchema(int id) {
        try {
            String registryUrl = appConfig.getKafka().getSchemaRegistryUrl().toString();
            if (registryUrl.endsWith("/")) {
                registryUrl = registryUrl.substring(0, registryUrl.length() - 1);
            }
            String url = registryUrl + "/schemas/ids/" + id;
            logger.info("Fetching schema ID {} from Schema Registry at {}", id, registryUrl);

            String response = restClient.get()
                    .uri(url)
                    .retrieve()
                    .body(String.class);

            if (response != null) {
                JsonNode node = objectMapper.readTree(response);
                String schemaStr = node.get("schema").asText();
                Schema schema = new Schema.Parser().parse(schemaStr);
                schemaCache.put(id, schema);
                logger.info("Cached schema for ID: {}", id);
            }
        } catch (Exception e) {
            logger.error("Failed to fetch schema {}", id, e);
        }
    }
}
