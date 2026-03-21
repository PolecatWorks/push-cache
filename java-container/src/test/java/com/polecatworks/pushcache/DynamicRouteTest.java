package com.polecatworks.pushcache;

import com.polecatworks.pushcache.service.Cache;
import com.polecatworks.pushcache.service.InMemoryCache;
import com.polecatworks.pushcache.service.SchemaService;
import org.apache.avro.Schema;
import org.apache.avro.generic.GenericDatumWriter;
import org.apache.avro.io.BinaryEncoder;
import org.apache.avro.io.EncoderFactory;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.test.context.TestPropertySource;
import org.springframework.test.web.reactive.server.WebTestClient;

import java.io.ByteArrayOutputStream;
import java.util.Collections;

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
        "cache.routes[0].path=/",
        "cache.routes[0].store=mem"
})
public class DynamicRouteTest {

    @Autowired
    private WebTestClient webTestClient;

    @Autowired
    private Cache cache;

    @MockBean
    private SchemaService schemaService;

    @org.junit.jupiter.api.BeforeEach
    void setup() {
        cache.clear().block();
    }

    @Test
    void testListRecords() throws Exception {
        cache.put("key1", "val1".getBytes()).block();
        cache.put("key2", "val2".getBytes()).block();

        webTestClient.get().uri("/api")
                .exchange()
                .expectStatus().isOk()
                .expectBody()
                .jsonPath("$.length()").isEqualTo(2)
                .jsonPath("$[0]").isEqualTo("key1")
                .jsonPath("$[1]").isEqualTo("key2");
    }

    @Test
    void testListRecordsNoTrailingSlash() throws Exception {
        cache.put("key1", "val1".getBytes()).block();
        cache.put("key2", "val2".getBytes()).block();

        // /api is configured as base path, so /api should work same as /api/
        webTestClient.get().uri("/api")
                .exchange()
                .expectStatus().isOk()
                .expectBody()
                .jsonPath("$.length()").isEqualTo(2)
                .jsonPath("$[0]").isEqualTo("key1")
                .jsonPath("$[1]").isEqualTo("key2");
    }

    @Test
    void testGetRecord() throws Exception {
        Schema schema = Schema.create(Schema.Type.STRING);
        when(schemaService.getSchema(anyInt())).thenReturn(schema);

        // Encode "hello" in Avro
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(0); // Magic
        out.write(0);
        out.write(0);
        out.write(0);
        out.write(1); // Schema ID 1

        BinaryEncoder encoder = EncoderFactory.get().binaryEncoder(out, null);
        GenericDatumWriter<Object> writer = new GenericDatumWriter<>(schema);
        writer.write("hello", encoder);
        encoder.flush();

        byte[] content = out.toByteArray();
        cache.put("key1", content).block();

        webTestClient.get().uri("/api/key1")
                .exchange()
                .expectStatus().isOk()
                .expectHeader().valueEquals("Cache-Control", "max-age=60, public")
                .expectBody().json("\"hello\"");
    }

    @Test
    void testGetRecordNotFound() throws Exception {
        webTestClient.get().uri("/api/unknown")
                .exchange()
                .expectStatus().isNotFound();
    }

    @Test
    void testDeleteRecord() throws Exception {
        byte[] content = "to_delete".getBytes();
        cache.put("del_key", content).block();

        webTestClient.delete().uri("/api/del_key")
                .exchange()
                .expectStatus().isOk()
                .expectBody(byte[].class).isEqualTo(content);

        // Ensure it is gone
        webTestClient.get().uri("/api/del_key")
                .exchange()
                .expectStatus().isNotFound();
    }

    @Test
    void testCreateRecordSuccess() throws Exception {
        byte[] body = new byte[10];
        body[0] = 0; // Magic byte
        // Schema ID 1 (Big Endian: 0 0 0 1)
        body[1] = 0;
        body[2] = 0;
        body[3] = 0;
        body[4] = 1;
        // Data
        body[5] = 0x10;

        webTestClient.post().uri("/api/new_record")
                .bodyValue(body)
                .exchange()
                .expectStatus().isCreated()
                .expectBody().jsonPath("$.id").isEqualTo("new_record");

        // Verify it is in cache
        byte[] cached = cache.get("new_record").block();
        org.junit.jupiter.api.Assertions.assertArrayEquals(body, cached);
    }

    @Test
    void testCreateRecordPayloadTooShort() throws Exception {
        byte[] body = new byte[4];
        webTestClient.post().uri("/api/short_record")
                .bodyValue(body)
                .exchange()
                .expectStatus().is5xxServerError()
                .expectBody().jsonPath("$.message").isEqualTo("Payload too short");
    }

    @Test
    void testCreateRecordInvalidMagicByte() throws Exception {
        byte[] body = new byte[10];
        body[0] = 1; // Invalid Magic byte

        webTestClient.post().uri("/api/bad_magic")
                .bodyValue(body)
                .exchange()
                .expectStatus().is5xxServerError()
                .expectBody().jsonPath("$.message").isEqualTo("Invalid Magic Byte");
    }
}
