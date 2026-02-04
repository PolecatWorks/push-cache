package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.config.StartupCheckConfig;
import org.apache.kafka.clients.consumer.Consumer;
import org.apache.kafka.clients.consumer.ConsumerConfig;
import org.apache.kafka.clients.consumer.KafkaConsumer;
import org.apache.kafka.common.PartitionInfo;
import org.apache.kafka.common.serialization.StringDeserializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.core.ParameterizedTypeReference;
import org.springframework.stereotype.Service;
import org.springframework.web.client.RestClient;

import java.time.Duration;
import java.util.List;
import java.util.Properties;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.function.Function;

@Service
public class StartupCheckService {

    private static final Logger logger = LoggerFactory.getLogger(StartupCheckService.class);
    private final AppConfig appConfig;
    private final RestClient restClient;
    private final Function<Properties, Consumer<String, String>> kafkaConsumerFactory;

    @Autowired
    public StartupCheckService(AppConfig appConfig) {
        this(appConfig, RestClient.builder().build(), props -> new KafkaConsumer<>(props));
    }

    public StartupCheckService(AppConfig appConfig, RestClient restClient, Function<Properties, Consumer<String, String>> kafkaConsumerFactory) {
        this.appConfig = appConfig;
        this.restClient = restClient;
        this.kafkaConsumerFactory = kafkaConsumerFactory;
    }

    public void runStartupChecks() throws Exception {
        StartupCheckConfig checkConfig = appConfig.getStartupChecks();

        if (!checkConfig.isEnabled()) {
            logger.info("Startup checks are disabled.");
            return;
        }

        CompletableFuture<Void> schemaRegistryCheck = CompletableFuture.runAsync(() ->
            runCheck("Schema Registry Connectivity", checkConfig, this::checkSchemaRegistry)
        );

        CompletableFuture<Void> kafkaCheck = CompletableFuture.runAsync(() ->
            runCheck("Kafka Metadata Connectivity", checkConfig, this::checkKafkaMetadata)
        );

        try {
            CompletableFuture.allOf(schemaRegistryCheck, kafkaCheck).get();
            logger.info("All startup checks passed.");
        } catch (ExecutionException e) {
            Throwable cause = e.getCause();
            if (cause instanceof Exception) {
                throw (Exception) cause;
            } else {
                throw new RuntimeException(cause);
            }
        }
    }

    void runCheck(String name, StartupCheckConfig config, Runnable check) {
        logger.info("Running check: {}", name);
        int attemptsRemaining = config.getFails();

        while (attemptsRemaining > 0) {
            try {
                check.run();
                logger.info("Check passed: {}", name);
                return;
            } catch (Exception e) {
                logger.warn("Check failed: {}, error= {} rerunning in {}", name, e.getMessage(), config.getTimeout());
            }

            attemptsRemaining--;
            if (attemptsRemaining > 0) {
                 logger.warn("Check failed: {}, {} attempts remaining, rerunning in {}", name, attemptsRemaining, config.getTimeout());
                 try {
                     Thread.sleep(config.getTimeout().toMillis());
                 } catch (InterruptedException ie) {
                     Thread.currentThread().interrupt();
                     throw new RuntimeException("Interrupted while waiting for retry", ie);
                 }
            }
        }
        throw new RuntimeException(String.format("Check %s failed after %d attempts", name, config.getFails()));
    }

    private void checkSchemaRegistry() {
        String url = appConfig.getKafka().getSchemaRegistryUrl().toString();
        if (url.endsWith("/")) {
            url = url.substring(0, url.length() - 1);
        }
        String checkUrl = url + "/schemas/types";
        logger.info("Checking Schema Registry at {}", checkUrl);

        List<String> types = restClient.get()
                .uri(checkUrl)
                .retrieve()
                .body(new ParameterizedTypeReference<List<String>>() {});

        if (types == null || !types.contains("AVRO")) {
             throw new RuntimeException("Schema type AVRO is not supported by the Schema Registry. Supported types: " + types);
        }
    }

    void checkKafkaMetadata() {
        Properties props = new Properties();
        String host = appConfig.getKafka().getBrokers().getHost();
        int port = appConfig.getKafka().getBrokers().getPort();
        if (host == null || port == -1) {
             throw new RuntimeException("Kafka broker host or port not defined in URI: " + appConfig.getKafka().getBrokers());
        }
        String brokerString = host + ":" + port;

        props.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, brokerString);
        props.put(ConsumerConfig.GROUP_ID_CONFIG, "startup-check-" + UUID.randomUUID());
        props.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, StringDeserializer.class);
        props.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, StringDeserializer.class);

        long timeoutMs = appConfig.getKafka().getFetchMetadataTimeout().toMillis();
        props.put(ConsumerConfig.REQUEST_TIMEOUT_MS_CONFIG, (int) timeoutMs);
        props.put(ConsumerConfig.DEFAULT_API_TIMEOUT_MS_CONFIG, (int) timeoutMs);

        try (Consumer<String, String> consumer = kafkaConsumerFactory.apply(props)) {
            String topic = appConfig.getKafka().getTopic();
            List<PartitionInfo> partitions = consumer.partitionsFor(topic, Duration.ofMillis(timeoutMs));

            if (partitions == null || partitions.isEmpty()) {
                logger.warn("Kafka topic {} not found or has no partitions", topic);
                throw new RuntimeException("Kafka topic " + topic + " not found or has no partitions");
            }
        }
    }
}
