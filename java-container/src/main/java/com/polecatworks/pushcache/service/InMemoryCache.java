package com.polecatworks.pushcache.service;

import reactor.core.publisher.Flux;
import reactor.core.publisher.Mono;

import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;
import java.util.Map;

public class InMemoryCache implements Cache {
    private final Map<String, byte[]> store = new ConcurrentHashMap<>();
    private final MetricsService metricsService;
    private final String name;
    private final AtomicLong size = new AtomicLong(0);

    public InMemoryCache(String name, MetricsService metricsService) {
        this.name = name;
        this.metricsService = metricsService;
        this.metricsService.registerCacheSize(name, size);
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public Mono<Void> put(String key, byte[] value) {
        return Mono.fromRunnable(() -> {
            if (store.put(key, value) == null) {
                size.incrementAndGet();
            }
        });
    }

    @Override
    public Mono<byte[]> get(String key) {
        return Mono.justOrEmpty(store.get(key));
    }

    @Override
    public Mono<byte[]> remove(String key) {
        return Mono.fromCallable(() -> {
            byte[] val = store.remove(key);
            if (val != null) {
                size.decrementAndGet();
            }
            return val;
        });
    }

    @Override
    public Flux<String> getKeys() {
        return Flux.fromIterable(store.keySet());
    }

    @Override
    public Mono<Boolean> containsKey(String key) {
        return Mono.just(store.containsKey(key));
    }

    @Override
    public Mono<Void> clear() {
        return Mono.fromRunnable(() -> {
            store.clear();
            size.set(0);
        });
    }

    @Override
    public Mono<Void> checkHealth() {
        return Mono.empty();
    }
}
