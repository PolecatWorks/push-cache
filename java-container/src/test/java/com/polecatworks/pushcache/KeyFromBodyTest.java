package com.polecatworks.pushcache;

import com.polecatworks.pushcache.service.Cache;
import com.polecatworks.pushcache.service.SchemaService;
import org.apache.avro.Schema;
import org.apache.avro.generic.GenericDatumWriter;
import org.apache.avro.io.BinaryEncoder;
import org.apache.avro.io.EncoderFactory;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.http.MediaType;
import org.springframework.test.context.TestPropertySource;
import org.springframework.test.web.reactive.server.WebTestClient;

import java.io.ByteArrayOutputStream;

import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.Mockito.when;

@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@TestPropertySource(properties = {
        // Hams
        "hams.address=0.0.0.0:8079",
        "hams.prefix=hams",
        "hams.logging=true",
        "hams.checks.timeout=5",
        "hams.checks.fails=2",
        "hams.checks.preflights=",
        "hams.checks.shutdowns=",

        // Runtime
        "runtime.threads=1",
        "runtime.stack-size=1024",
        "runtime.name=test",

        // WebService
        "webservice.address=http://localhost:0/api",

        // Kafka
        "kafka.brokers=tcp://localhost:9092",
        "kafka.group-id=test",
        "kafka.topic=test",
        "kafka.schema-registry-url=http://localhost:8081",
        "kafka.cache-max-age=60s",
        "kafka.fetch-metadata-timeout=5s",
        "kafka.offset-reset=earliest",
        "kafka.force-reset-earliest=false",

        // Startup Checks
        "startup-checks.fails=1",
        "startup-checks.timeout=100ms",
        "startup-checks.enabled=false",

        // Cache
        "cache.stores[0].name=mem",
        "cache.stores[0].type=in_memory",
        "cache.routes[0].path=/users",
        "cache.routes[0].store=mem",
        "cache.routes[0].key-from-body=userId"
})
public class KeyFromBodyTest {

    @Autowired
    private WebTestClient webTestClient;

    @Autowired
    private Cache cache;

    @MockBean
    private SchemaService schemaService;

    @BeforeEach
    void setup() {
        cache.clear().block();
    }

    private void populateCache(String key, String value) throws Exception {
        Schema schema = Schema.create(Schema.Type.STRING);
        when(schemaService.getSchema(anyInt())).thenReturn(schema);

        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(0); // Magic
        out.write(0);
        out.write(0);
        out.write(0);
        out.write(1); // Schema ID 1

        BinaryEncoder encoder = EncoderFactory.get().binaryEncoder(out, null);
        GenericDatumWriter<Object> writer = new GenericDatumWriter<>(schema);
        writer.write(value, encoder);
        encoder.flush();

        cache.put(key, out.toByteArray()).block();
    }

    @Test
    void testGetRecordByBodySuccess() throws Exception {
        populateCache("user123", "Alice");

        String jsonBody = "{\"userId\": \"user123\"}";

        webTestClient.method(org.springframework.http.HttpMethod.GET)
                .uri("/api/users_by_body")
                .contentType(MediaType.APPLICATION_JSON)
                .bodyValue(jsonBody)
                .exchange()
                .expectStatus().isOk()
                .expectBody().json("\"Alice\"");
    }

    @Test
    void testGetRecordByBodyMissingKey() throws Exception {
        String jsonBody = "{\"otherId\": \"user123\"}";

        webTestClient.method(org.springframework.http.HttpMethod.GET)
                .uri("/api/users_by_body")
                .contentType(MediaType.APPLICATION_JSON)
                .bodyValue(jsonBody)
                .exchange()
                .expectStatus().isBadRequest()
                .expectBody().jsonPath("$.message").isEqualTo("Missing key 'userId' in body");
    }

    @Test
    void testGetRecordByBodyInvalidJson() throws Exception {
        String jsonBody = "{ invalid json }";

        webTestClient.method(org.springframework.http.HttpMethod.GET)
                .uri("/api/users_by_body")
                .contentType(MediaType.APPLICATION_JSON)
                .bodyValue(jsonBody)
                .exchange()
                .expectStatus().isBadRequest();
    }

    @Test
    void testGetRecordByBodyNotFound() throws Exception {
        String jsonBody = "{\"userId\": \"unknown\"}";

        webTestClient.method(org.springframework.http.HttpMethod.GET)
                .uri("/api/users_by_body")
                .contentType(MediaType.APPLICATION_JSON)
                .bodyValue(jsonBody)
                .exchange()
                .expectStatus().isNotFound()
                .expectBody().jsonPath("$.message").isEqualTo("User not found in dynamic cache");
    }

    @Test
    void testGetRecordByBodyKeyNotString() throws Exception {
        String jsonBody = "{\"userId\": true}";

        webTestClient.method(org.springframework.http.HttpMethod.GET)
                .uri("/api/users_by_body")
                .contentType(MediaType.APPLICATION_JSON)
                .bodyValue(jsonBody)
                .exchange()
                .expectStatus().isBadRequest()
                .expectBody().jsonPath("$.message").isEqualTo("Key 'userId' must be a string or number");
    }

    @Test
    void testStandardRouteStillWorks() throws Exception {
        populateCache("user123", "Alice");

        webTestClient.get()
                .uri("/api/users/user123")
                .exchange()
                .expectStatus().isOk()
                .expectBody().json("\"Alice\"");
    }
}
