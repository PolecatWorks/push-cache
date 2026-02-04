package com.polecatworks.pushcache.web;

import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.service.CacheStore;
import org.springframework.http.CacheControl;
import org.springframework.http.MediaType;
import org.springframework.stereotype.Component;
import org.springframework.web.servlet.function.ServerRequest;
import org.springframework.web.servlet.function.ServerResponse;

import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.TimeUnit;
import java.util.stream.Collectors;

@Component
public class RecordHandler {

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
