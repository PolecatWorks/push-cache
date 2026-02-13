package com.polecatworks.pushcache.service;

import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.Map;

public class InMemoryCache implements Cache {
    private final Map<String, byte[]> store = new ConcurrentHashMap<>();
    private final MetricsService metricsService;
    private final String name;

    public InMemoryCache(String name, MetricsService metricsService) {
        this.name = name;
        this.metricsService = metricsService;
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public void put(String key, byte[] value) {
        if (store.put(key, value) == null) {
            metricsService.incrementCacheSize();
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
            metricsService.decrementCacheSize();
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
        metricsService.setCacheSize(0);
    }

    @Override
    public void checkHealth() throws Exception {
        // Always healthy
    }
}
