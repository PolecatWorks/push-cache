package com.polecatworks.pushcache;

import com.polecatworks.pushcache.service.CacheStore;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.context.TestPropertySource;
import org.springframework.test.web.servlet.MockMvc;

import static org.hamcrest.Matchers.contains;
import static org.hamcrest.Matchers.hasSize;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.delete;
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

    // Runtime
    "runtime.threads=1",
    "runtime.stack-size=1024",
    "runtime.name=test",

    // WebService
    "webservice.address=http://localhost:8080/api",
    "webservice.path-dynamic=dynamic",

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
    "startup-checks.enabled=false"
})
public class DynamicRouteTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private CacheStore cacheStore;

    @Test
    void testListRecords() throws Exception {
        cacheStore.put("key1", "val1".getBytes());
        cacheStore.put("key2", "val2".getBytes());

        mockMvc.perform(get("/api/"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$", hasSize(2)))
                .andExpect(jsonPath("$", contains("key1", "key2")));
    }

    @Test
    void testGetRecord() throws Exception {
        byte[] content = "hello".getBytes();
        cacheStore.put("key1", content);

        mockMvc.perform(get("/api/key1"))
                .andExpect(status().isOk())
                .andExpect(header().string("Cache-Control", "max-age=60, public"))
                .andExpect(content().bytes(content));
    }

    @Test
    void testGetRecordNotFound() throws Exception {
        mockMvc.perform(get("/api/unknown"))
                .andExpect(status().isNotFound());
    }

    @Test
    void testDeleteRecord() throws Exception {
        byte[] content = "to_delete".getBytes();
        cacheStore.put("del_key", content);

        mockMvc.perform(delete("/api/del_key"))
                .andExpect(status().isOk())
                .andExpect(content().bytes(content));

        // Ensure it is gone
        mockMvc.perform(get("/api/del_key"))
                .andExpect(status().isNotFound());
    }
}
