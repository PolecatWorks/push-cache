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
import org.springframework.test.web.servlet.MockMvc;

import java.io.ByteArrayOutputStream;
import java.util.Collections;

import static org.hamcrest.Matchers.contains;
import static org.hamcrest.Matchers.hasSize;
import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.delete;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultHandlers.print;
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
        "cache.routes[0].path=/",
        "cache.routes[0].store=mem"
})
public class DynamicRouteTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private Cache cache;

    @MockBean
    private SchemaService schemaService;

    @org.junit.jupiter.api.BeforeEach
    void setup() {
        cache.clear();
    }

    @Test
    void testListRecords() throws Exception {
        cache.put("key1", "val1".getBytes());
        cache.put("key2", "val2".getBytes());

        mockMvc.perform(get("/api"))
                .andDo(print())
                .andExpect(status().isOk())
                .andExpect(jsonPath("$", hasSize(2)))
                .andExpect(jsonPath("$", contains("key1", "key2")));
    }

    @Test
    void testListRecordsNoTrailingSlash() throws Exception {
        cache.put("key1", "val1".getBytes());
        cache.put("key2", "val2".getBytes());

        // /api is configured as base path, so /api should work same as /api/
        mockMvc.perform(get("/api"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$", hasSize(2)))
                .andExpect(jsonPath("$", contains("key1", "key2")));
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
        cache.put("key1", content);

        mockMvc.perform(get("/api/key1"))
                .andExpect(status().isOk())
                .andExpect(header().string("Cache-Control", "max-age=60, public"))
                .andExpect(content().json("\"hello\""));
    }

    @Test
    void testGetRecordNotFound() throws Exception {
        mockMvc.perform(get("/api/unknown"))
                .andExpect(status().isNotFound());
    }

    @Test
    void testDeleteRecord() throws Exception {
        byte[] content = "to_delete".getBytes();
        cache.put("del_key", content);

        mockMvc.perform(delete("/api/del_key"))
                .andExpect(status().isOk())
                .andExpect(content().bytes(content));

        // Ensure it is gone
        mockMvc.perform(get("/api/del_key"))
                .andExpect(status().isNotFound());
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

        mockMvc.perform(post("/api/new_record")
                .content(body))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").value("new_record"));

        // Verify it is in cache
        byte[] cached = cache.get("new_record");
        org.junit.jupiter.api.Assertions.assertArrayEquals(body, cached);
    }

    @Test
    void testCreateRecordPayloadTooShort() throws Exception {
        byte[] body = new byte[4];
        mockMvc.perform(post("/api/short_record")
                .content(body))
                .andExpect(status().isInternalServerError())
                .andExpect(jsonPath("$.message").value("Payload too short"));
    }

    @Test
    void testCreateRecordInvalidMagicByte() throws Exception {
        byte[] body = new byte[10];
        body[0] = 1; // Invalid Magic byte

        mockMvc.perform(post("/api/bad_magic")
                .content(body))
                .andExpect(status().isInternalServerError())
                .andExpect(jsonPath("$.message").value("Invalid Magic Byte"));
    }
}
