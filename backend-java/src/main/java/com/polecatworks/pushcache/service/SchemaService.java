package com.polecatworks.pushcache.service;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.polecatworks.pushcache.config.AppConfig;
import org.apache.avro.Schema;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;
import org.springframework.web.client.RestClient;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

@Service
public class SchemaService {
    private static final Logger logger = LoggerFactory.getLogger(SchemaService.class);
    private final AppConfig appConfig;
    private final RestClient restClient;
    private final Map<Integer, Schema> schemaCache = new ConcurrentHashMap<>();
    private final ObjectMapper objectMapper = new ObjectMapper();

    @org.springframework.beans.factory.annotation.Autowired
    public SchemaService(AppConfig appConfig) {
        this(appConfig, RestClient.builder().build());
    }

    public SchemaService(AppConfig appConfig, RestClient restClient) {
        this.appConfig = appConfig;
        this.restClient = restClient;
    }

    public Schema getSchema(int id) {
        if (schemaCache.containsKey(id)) {
            return schemaCache.get(id);
        }
        return fetchAndCacheSchema(id);
    }

    private synchronized Schema fetchAndCacheSchema(int id) {
        if (schemaCache.containsKey(id)) {
            return schemaCache.get(id);
        }

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
                return schema;
            }
        } catch (Exception e) {
            logger.error("Failed to fetch schema {}", id, e);
            throw new RuntimeException("Failed to fetch schema " + id, e);
        }
        throw new RuntimeException("Schema not found for ID " + id);
    }
}
