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
import org.springframework.test.web.servlet.MockMvc;

import java.io.ByteArrayOutputStream;

import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@SpringBootTest
@AutoConfigureMockMvc
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
        "webservice.address=http://localhost:8080/api",

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
    private MockMvc mockMvc;

    @Autowired
    private Cache cache;

    @MockBean
    private SchemaService schemaService;

    @BeforeEach
    void setup() {
        cache.clear();
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

        cache.put(key, out.toByteArray());
    }

    @Test
    void testGetRecordByBodySuccess() throws Exception {
        populateCache("user123", "Alice");

        String jsonBody = "{\"userId\": \"user123\"}";

        mockMvc.perform(get("/api/users_by_body")
                .contentType(MediaType.APPLICATION_JSON)
                .content(jsonBody))
                .andExpect(status().isOk())
                .andExpect(content().json("\"Alice\""));
    }

    @Test
    void testGetRecordByBodyMissingKey() throws Exception {
        String jsonBody = "{\"otherId\": \"user123\"}";

        mockMvc.perform(get("/api/users_by_body")
                .contentType(MediaType.APPLICATION_JSON)
                .content(jsonBody))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.message").value("Missing key 'userId' in body"));
    }

    @Test
    void testGetRecordByBodyInvalidJson() throws Exception {
        String jsonBody = "{ invalid json }";

        mockMvc.perform(get("/api/users_by_body")
                .contentType(MediaType.APPLICATION_JSON)
                .content(jsonBody))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.message").value("Invalid JSON body"));
    }

    @Test
    void testGetRecordByBodyNotFound() throws Exception {
        String jsonBody = "{\"userId\": \"unknown\"}";

        mockMvc.perform(get("/api/users_by_body")
                .contentType(MediaType.APPLICATION_JSON)
                .content(jsonBody))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.message").value("User not found in dynamic cache"));
    }

    @Test
    void testGetRecordByBodyKeyNotString() throws Exception {
        String jsonBody = "{\"userId\": true}";

        mockMvc.perform(get("/api/users_by_body")
                .contentType(MediaType.APPLICATION_JSON)
                .content(jsonBody))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.message").value("Key 'userId' must be a string or number"));
    }

    @Test
    void testStandardRouteStillWorks() throws Exception {
        populateCache("user123", "Alice");

        mockMvc.perform(get("/api/users/user123"))
                .andExpect(status().isOk())
                .andExpect(content().json("\"Alice\""));
    }
}
