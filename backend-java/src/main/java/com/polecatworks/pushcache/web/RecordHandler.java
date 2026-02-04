package com.polecatworks.pushcache.web;

import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.service.CacheStore;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.CacheControl;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.stereotype.Component;
import org.springframework.web.servlet.function.ServerRequest;
import org.springframework.web.servlet.function.ServerResponse;

import java.nio.ByteBuffer;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.stream.Collectors;

@Component
public class RecordHandler {

    private static final Logger logger = LoggerFactory.getLogger(RecordHandler.class);

    private final CacheStore cacheStore;
    private final AppConfig appConfig;

    public RecordHandler(CacheStore cacheStore, AppConfig appConfig) {
        this.cacheStore = cacheStore;
        this.appConfig = appConfig;
    }

    public ServerResponse listRecords(ServerRequest request) {
        Optional<String> filter = request.param("filter");
        Optional<String> limitParam = request.param("limit");
        Optional<String> offsetParam = request.param("offset");

        List<String> keys = new ArrayList<>(cacheStore.getKeys());

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

        cacheStore.put(id, body);

        return ServerResponse.status(HttpStatus.CREATED)
                .contentType(MediaType.APPLICATION_JSON)
                .body(Collections.singletonMap("id", id));
    }

    public ServerResponse getRecord(ServerRequest request) {
        String id = request.pathVariable("id");
        byte[] data = cacheStore.get(id);

        if (data == null) {
            return ServerResponse.notFound().build();
        }

        Duration maxAge = appConfig.getKafka().getCacheMaxAge();

        return ServerResponse.ok()
                .cacheControl(CacheControl.maxAge(maxAge).cachePublic())
                .body(data);
    }

    public ServerResponse deleteRecord(ServerRequest request) {
        String id = request.pathVariable("id");
        byte[] data = cacheStore.remove(id);

        if (data == null) {
            return ServerResponse.notFound().build();
        }

        return ServerResponse.ok().body(data);
    }
}
