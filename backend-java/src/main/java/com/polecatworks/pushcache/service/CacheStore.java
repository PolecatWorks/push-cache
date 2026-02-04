package com.polecatworks.pushcache.service;

import org.springframework.stereotype.Service;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.Map;

@Service
public class CacheStore {
    private final Map<String, byte[]> store = new ConcurrentHashMap<>();

    public void put(String key, byte[] value) {
        store.put(key, value);
    }

    public byte[] get(String key) {
        return store.get(key);
    }

    public byte[] remove(String key) {
        return store.remove(key);
    }

    public Set<String> getKeys() {
        return store.keySet();
    }
}
