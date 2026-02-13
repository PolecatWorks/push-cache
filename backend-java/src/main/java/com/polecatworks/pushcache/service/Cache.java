package com.polecatworks.pushcache.service;

import java.util.Set;

public interface Cache {
    String getName();
    void put(String key, byte[] value);
    byte[] get(String key);
    byte[] remove(String key);
    Set<String> getKeys();
    boolean containsKey(String key);
    void clear();
}
