package com.polecatworks.pushcache.service;

import java.util.Set;
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
    public void put(String key, byte[] value) {
        if (store.put(key, value) == null) {
            size.incrementAndGet();
        }
    }

    @Override
    public byte[] get(String key) {
        return store.get(key);
    }

    @Override
    public byte[] remove(String key) {
        byte[] val = store.remove(key);
        if (val != null) {
            size.decrementAndGet();
        }
        return val;
    }

    @Override
    public Set<String> getKeys() {
        return store.keySet();
    }

    @Override
    public boolean containsKey(String key) {
        return store.containsKey(key);
    }

    @Override
    public void clear() {
        store.clear();
        size.set(0);
    }

    @Override
    public void checkHealth() throws Exception {
        // Always healthy
    }
}
