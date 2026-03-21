package com.polecatworks.pushcache.service;

import reactor.core.publisher.Flux;
import reactor.core.publisher.Mono;

public interface Cache {
    String getName();
    Mono<Void> put(String key, byte[] value);
    Mono<byte[]> get(String key);
    Mono<byte[]> remove(String key);
    Flux<String> getKeys();
    Mono<Boolean> containsKey(String key);
    Mono<Void> clear();
    Mono<Void> checkHealth();
}
