package com.polecatworks.pushcache.service;

import org.springframework.stereotype.Service;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.Map;

@Service
public class CacheStore {
    private final Map<String, byte[]> store = new ConcurrentHashMap<>();
    private final MetricsService metricsService;

    public CacheStore(MetricsService metricsService) {
        this.metricsService = metricsService;
    }

    public void put(String key, byte[] value) {
        if (store.put(key, value) == null) {
            metricsService.incrementCacheSize();
        }
    }

    public byte[] get(String key) {
        return store.get(key);
    }

    public byte[] remove(String key) {
        byte[] val = store.remove(key);
        if (val != null) {
            metricsService.decrementCacheSize();
        }
        return val;
    }

    public Set<String> getKeys() {
        return store.keySet();
    }
}
