package com.polecatworks.pushcache.web;

import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.service.Cache;
import com.polecatworks.pushcache.service.MetricsService;
import com.polecatworks.pushcache.service.SchemaService;
import org.apache.avro.Schema;
import org.apache.avro.generic.GenericDatumReader;
import org.apache.avro.generic.GenericDatumWriter;
import org.apache.avro.io.BinaryDecoder;
import org.apache.avro.io.DecoderFactory;
import org.apache.avro.io.EncoderFactory;
import org.apache.avro.io.JsonEncoder;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.CacheControl;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.web.servlet.function.ServerRequest;
import org.springframework.web.servlet.function.ServerResponse;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.stream.Collectors;

public class RecordHandler {

    private static final Logger logger = LoggerFactory.getLogger(RecordHandler.class);

    private final Cache cache;
    private final AppConfig appConfig;
    private final SchemaService schemaService;
    private final MetricsService metricsService;
    private final String keyFromBody;

    public RecordHandler(Cache cache, AppConfig appConfig, SchemaService schemaService,
            MetricsService metricsService) {
        this(cache, appConfig, schemaService, metricsService, null);
    }

    public RecordHandler(Cache cache, AppConfig appConfig, SchemaService schemaService,
            MetricsService metricsService, String keyFromBody) {
        this.cache = cache;
        this.appConfig = appConfig;
        this.schemaService = schemaService;
        this.metricsService = metricsService;
        this.keyFromBody = keyFromBody;
    }

    public ServerResponse listRecords(ServerRequest request) {
        Optional<String> filter = request.param("filter");
        Optional<String> limitParam = request.param("limit");
        Optional<String> offsetParam = request.param("offset");

        List<String> keys = new ArrayList<>(cache.getKeys());

        if (filter.isPresent()) {
            String filterVal = filter.get();
            keys.removeIf(k -> !k.contains(filterVal));
        }

        Collections.sort(keys);

        long offset = 0;
        long limit = Long.MAX_VALUE;

        try {
            offset = offsetParam.map(Long::parseLong).orElse(0L);
            limit = limitParam.map(Long::parseLong).orElse(Long.MAX_VALUE);
        } catch (NumberFormatException e) {
            return ServerResponse.badRequest().body("Invalid limit or offset");
        }

        List<String> pagedKeys = keys.stream()
                .skip(offset)
                .limit(limit)
                .collect(Collectors.toList());

        System.err.println("Returning " + pagedKeys.size() + " keys");

        return ServerResponse.ok()
                .contentType(MediaType.APPLICATION_JSON)
                .body(pagedKeys);
    }

    public ServerResponse createRecord(ServerRequest request) {
        String id = request.pathVariable("id");
        byte[] body;
        try {
            body = request.body(byte[].class);
        } catch (Exception e) {
            return ServerResponse.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(Collections.singletonMap("message", "Failed to read body"));
        }

        if (body.length < 5) {
            return ServerResponse.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(Collections.singletonMap("message", "Payload too short"));
        }

        if (body[0] != 0) {
            return ServerResponse.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(Collections.singletonMap("message", "Invalid Magic Byte"));
        }

        int schemaId = ByteBuffer.wrap(body, 1, 4).getInt();
        logger.info("Received record with Schema ID: {}", schemaId);

        cache.put(id, body);

        return ServerResponse.status(HttpStatus.CREATED)
                .contentType(MediaType.APPLICATION_JSON)
                .body(Collections.singletonMap("id", id));
    }

    public ServerResponse getRecord(ServerRequest request) {
        metricsService.incrementRequestsTotal(cache.getName());
        String id = request.pathVariable("id");
        return retrieveRecord(id);
    }

    @SuppressWarnings("unchecked")
    public ServerResponse getRecordByBody(ServerRequest request) {
        metricsService.incrementRequestsTotal(cache.getName());

        Map<String, Object> body;
        try {
            body = request.body(Map.class);
        } catch (Exception e) {
            return ServerResponse.badRequest()
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(Collections.singletonMap("message", "Invalid JSON body"));
        }

        if (keyFromBody == null) {
            return ServerResponse.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(Collections.singletonMap("message", "key_from_body not configured"));
        }

        if (!body.containsKey(keyFromBody)) {
            return ServerResponse.badRequest()
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(Collections.singletonMap("message", "Missing key '" + keyFromBody + "' in body"));
        }

        Object keyVal = body.get(keyFromBody);
        String id;
        if (keyVal instanceof String) {
            id = (String) keyVal;
        } else if (keyVal instanceof Number) {
            id = keyVal.toString();
        } else {
            return ServerResponse.badRequest()
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(Collections.singletonMap("message", "Key '" + keyFromBody + "' must be a string or number"));
        }

        return retrieveRecord(id);
    }

    private ServerResponse retrieveRecord(String id) {
        byte[] data = cache.get(id);

        if (data == null) {
            metricsService.incrementRequestsMiss(cache.getName());
            return ServerResponse.status(HttpStatus.NOT_FOUND)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(Collections.singletonMap("message", "User not found in dynamic cache"));
        }

        try {
            if (data.length < 5) {
                throw new RuntimeException("Invalid Avro message format");
            }
            ByteBuffer buffer = ByteBuffer.wrap(data);
            byte magic = buffer.get();
            if (magic != 0) {
                throw new RuntimeException("Invalid Avro message format");
            }
            int schemaId = buffer.getInt();

            Schema schema;
            try {
                schema = schemaService.getSchema(schemaId);
            } catch (Exception e) {
                return ServerResponse.status(HttpStatus.NOT_FOUND)
                        .contentType(MediaType.APPLICATION_JSON)
                        .body(Collections.singletonMap("message", "Schema not found in cache"));
            }

            int binaryStart = buffer.position();
            int binaryLen = data.length - binaryStart;

            GenericDatumReader<Object> reader = new GenericDatumReader<>(schema);
            BinaryDecoder decoder = DecoderFactory.get().binaryDecoder(data, binaryStart, binaryLen, null);
            Object datum = reader.read(null, decoder);

            ByteArrayOutputStream out = new ByteArrayOutputStream();
            JsonEncoder encoder = EncoderFactory.get().jsonEncoder(schema, out);
            GenericDatumWriter<Object> writer = new GenericDatumWriter<>(schema);
            writer.write(datum, encoder);
            encoder.flush();
            byte[] jsonBytes = out.toByteArray();

            Duration maxAge = appConfig.getKafka().getCacheMaxAge();

            return ServerResponse.ok()
                    .contentType(MediaType.APPLICATION_JSON)
                    .cacheControl(CacheControl.maxAge(maxAge).cachePublic())
                    .body(jsonBytes);

        } catch (Exception e) {
            logger.error("Error processing record {}", id, e);
            return ServerResponse.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(Collections.singletonMap("message",
                            e.getMessage() != null ? e.getMessage() : "Avro Deserialization Error"));
        }
    }

    public ServerResponse deleteRecord(ServerRequest request) {
        String id = request.pathVariable("id");
        byte[] data = cache.remove(id);

        if (data == null) {
            return ServerResponse.status(HttpStatus.NOT_FOUND)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(Collections.singletonMap("message", "User not found"));
        }

        return ServerResponse.ok().body(data);
    }
}
