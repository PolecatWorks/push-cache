package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.config.KafkaConfig;
import com.polecatworks.pushcache.config.StartupCheckConfig;
import org.apache.kafka.clients.consumer.Consumer;
import org.apache.kafka.common.PartitionInfo;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.Mock;
import org.mockito.MockitoAnnotations;
import org.springframework.web.client.RestClient;

import java.net.URI;
import java.time.Duration;
import java.util.Collections;
import java.util.List;
import java.util.Properties;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Function;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

class StartupCheckServiceTest {

    @Mock
    private AppConfig appConfig;
    @Mock
    private RestClient restClient;
    @Mock
    private Consumer<String, String> kafkaConsumer;

    private StartupCheckService service;

    @BeforeEach
    void setUp() {
        MockitoAnnotations.openMocks(this);
        Function<Properties, Consumer<String, String>> consumerFactory = props -> kafkaConsumer;
        service = new StartupCheckService(appConfig, restClient, consumerFactory);
    }

    @Test
    void testRunCheckSuccessFirstTry() {
        StartupCheckConfig config = new StartupCheckConfig();
        config.setFails(3);
        config.setTimeout(Duration.ofMillis(1));

        Runnable check = mock(Runnable.class);

        assertDoesNotThrow(() -> service.runCheck("test", config, check));
        verify(check, times(1)).run();
    }

    @Test
    void testRunCheckSuccessOnRetry() {
        StartupCheckConfig config = new StartupCheckConfig();
        config.setFails(3);
        config.setTimeout(Duration.ofMillis(1));

        AtomicInteger attempts = new AtomicInteger(0);
        Runnable check = () -> {
            if (attempts.incrementAndGet() < 2) {
                throw new RuntimeException("Fail");
            }
        };

        assertDoesNotThrow(() -> service.runCheck("test", config, check));
        // Should succeed on 2nd attempt
    }

    @Test
    void testRunCheckFailure() {
        StartupCheckConfig config = new StartupCheckConfig();
        config.setFails(3);
        config.setTimeout(Duration.ofMillis(1));

        Runnable check = () -> {
             throw new RuntimeException("Fail");
        };

        assertThrows(RuntimeException.class, () -> service.runCheck("test", config, check));
    }

    @Test
    void testCheckKafkaMetadataSuccess() throws Exception {
        KafkaConfig kafkaConfig = new KafkaConfig();
        kafkaConfig.setBrokers(new URI("kafka://mybroker:9092"));
        kafkaConfig.setTopic("test-topic");
        kafkaConfig.setFetchMetadataTimeout(Duration.ofSeconds(1));

        when(appConfig.getKafka()).thenReturn(kafkaConfig);

        PartitionInfo partitionInfo = new PartitionInfo("test-topic", 0, null, null, null);
        when(kafkaConsumer.partitionsFor(eq("test-topic"), any())).thenReturn(List.of(partitionInfo));

        assertDoesNotThrow(() -> service.checkKafkaMetadata());
    }

    @Test
    void testCheckKafkaMetadataNoPartitions() throws Exception {
        KafkaConfig kafkaConfig = new KafkaConfig();
        kafkaConfig.setBrokers(new URI("kafka://mybroker:9092"));
        kafkaConfig.setTopic("test-topic");
        kafkaConfig.setFetchMetadataTimeout(Duration.ofSeconds(1));

        when(appConfig.getKafka()).thenReturn(kafkaConfig);

        when(kafkaConsumer.partitionsFor(eq("test-topic"), any())).thenReturn(Collections.emptyList());

        assertThrows(RuntimeException.class, () -> service.checkKafkaMetadata());
    }
}
