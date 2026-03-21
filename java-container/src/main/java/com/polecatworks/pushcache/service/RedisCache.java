package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.StoreDefinition;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.connection.RedisStandaloneConfiguration;
import org.springframework.data.redis.connection.lettuce.LettuceConnectionFactory;
import org.springframework.data.redis.core.ReactiveRedisTemplate;
import org.springframework.data.redis.core.ScanOptions;
import org.springframework.data.redis.serializer.RedisSerializationContext;
import org.springframework.data.redis.serializer.RedisSerializer;
import org.springframework.data.redis.serializer.StringRedisSerializer;
import reactor.core.publisher.Flux;
import reactor.core.publisher.Mono;

import java.net.URI;

public class RedisCache implements Cache, AutoCloseable {

    private static final Logger logger = LoggerFactory.getLogger(RedisCache.class);

    private final String name;
    private final String prefix;
    private final LettuceConnectionFactory connectionFactory;
    private final ReactiveRedisTemplate<String, byte[]> redisTemplate;

    public RedisCache(StoreDefinition storeDefinition) {
        this.name = storeDefinition.getName();
        this.prefix = storeDefinition.getPrefix();

        URI uri = storeDefinition.getUrl();
        if (uri == null) {
            throw new IllegalArgumentException("Redis URL must be provided for store: " + name);
        }

        RedisStandaloneConfiguration redisConfig = new RedisStandaloneConfiguration();
        redisConfig.setHostName(uri.getHost());
        redisConfig.setPort(uri.getPort() == -1 ? 6379 : uri.getPort());

        // Handle database index from path
        String path = uri.getPath();
        if (path != null && path.length() > 1) {
            try {
                int dbIndex = Integer.parseInt(path.substring(1));
                redisConfig.setDatabase(dbIndex);
            } catch (NumberFormatException e) {
                logger.warn("Could not parse database index from path '{}' for store '{}'. Using default 0.", path, name);
            }
        }

        // Handle password from userInfo
        String userInfo = uri.getUserInfo();
        if (userInfo != null) {
            String[] parts = userInfo.split(":", 2);
            if (parts.length > 1) {
                redisConfig.setPassword(parts[1]);
                if (!parts[0].isEmpty()) {
                    redisConfig.setUsername(parts[0]);
                }
            } else {
                redisConfig.setPassword(userInfo);
            }
        }

        this.connectionFactory = new LettuceConnectionFactory(redisConfig);
        this.connectionFactory.afterPropertiesSet();

        RedisSerializationContext<String, byte[]> serializationContext = RedisSerializationContext
                .<String, byte[]>newSerializationContext(new StringRedisSerializer())
                .value(RedisSerializer.byteArray())
                .hashKey(new StringRedisSerializer())
                .hashValue(RedisSerializer.byteArray())
                .build();

        this.redisTemplate = new ReactiveRedisTemplate<>(this.connectionFactory, serializationContext);

        logger.info("Initialized RedisCache '{}' connected to {}", name, uri);
    }

    private String formatKey(String key) {
        if (prefix != null && !prefix.isEmpty()) {
            return prefix + ":" + key;
        }
        return key;
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public Mono<Void> put(String key, byte[] value) {
        return redisTemplate.opsForValue().set(formatKey(key), value).then();
    }

    @Override
    public Mono<byte[]> get(String key) {
        return redisTemplate.opsForValue().get(formatKey(key));
    }

    @Override
    public Mono<byte[]> remove(String key) {
        String fullKey = formatKey(key);
        // ReactiveRedisTemplate opsForValue() has delete(K key) which returns Mono<Boolean>
        // We need to return the value that was deleted.
        return redisTemplate.opsForValue().get(fullKey)
                .flatMap(val -> redisTemplate.opsForValue().delete(fullKey).thenReturn(val));
    }

    @Override
    public Flux<String> getKeys() {
        String scanPattern = (prefix != null && !prefix.isEmpty()) ? prefix + ":*" : "*";
        ScanOptions options = ScanOptions.scanOptions().match(scanPattern).count(100).build();

        return redisTemplate.scan(options)
                .map(key -> {
                    if (prefix != null && !prefix.isEmpty()) {
                        if (key.startsWith(prefix + ":")) {
                            return key.substring(prefix.length() + 1);
                        }
                    }
                    return key;
                });
    }

    @Override
    public Mono<Boolean> containsKey(String key) {
        return redisTemplate.hasKey(formatKey(key));
    }

    @Override
    public Mono<Void> clear() {
        String scanPattern = (prefix != null && !prefix.isEmpty()) ? prefix + ":*" : "*";
        ScanOptions options = ScanOptions.scanOptions().match(scanPattern).count(100).build();

        return redisTemplate.scan(options)
                .buffer(100) // Delete in batches
                .flatMap(keys -> redisTemplate.delete(Flux.fromIterable(keys)))
                .then();
    }

    @Override
    public Mono<Void> checkHealth() {
        return redisTemplate.getConnectionFactory().getReactiveConnection().ping()
                .flatMap(response -> {
                    if (!"PONG".equalsIgnoreCase(response)) {
                        return Mono.error(new RuntimeException("Redis PING failed for cache " + name + ": " + response));
                    }
                    return Mono.empty();
                }).then();
    }

    @Override
    public void close() {
        if (connectionFactory != null) {
            connectionFactory.destroy();
            logger.info("Closed Redis connection for cache '{}'", name);
        }
    }
}
