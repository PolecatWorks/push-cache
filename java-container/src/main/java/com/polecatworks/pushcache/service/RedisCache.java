package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.StoreDefinition;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.connection.RedisConnection;
import org.springframework.data.redis.connection.RedisStandaloneConfiguration;
import org.springframework.data.redis.connection.lettuce.LettuceConnectionFactory;
import org.springframework.data.redis.core.Cursor;
import org.springframework.data.redis.core.RedisTemplate;
import org.springframework.data.redis.core.ScanOptions;
import org.springframework.data.redis.serializer.RedisSerializer;
import org.springframework.data.redis.serializer.StringRedisSerializer;

import java.net.URI;
import java.util.HashSet;
import java.util.Set;

public class RedisCache implements Cache, AutoCloseable {

    private static final Logger logger = LoggerFactory.getLogger(RedisCache.class);

    private final String name;
    private final String prefix;
    private final LettuceConnectionFactory connectionFactory;
    private final RedisTemplate<String, byte[]> redisTemplate;

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

        this.redisTemplate = new RedisTemplate<>();
        this.redisTemplate.setConnectionFactory(this.connectionFactory);
        this.redisTemplate.setKeySerializer(new StringRedisSerializer());
        this.redisTemplate.setValueSerializer(RedisSerializer.byteArray());
        this.redisTemplate.setHashKeySerializer(new StringRedisSerializer());
        this.redisTemplate.setHashValueSerializer(RedisSerializer.byteArray());
        this.redisTemplate.afterPropertiesSet();

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
    public void put(String key, byte[] value) {
        redisTemplate.opsForValue().set(formatKey(key), value);
    }

    @Override
    public byte[] get(String key) {
        return redisTemplate.opsForValue().get(formatKey(key));
    }

    @Override
    public byte[] remove(String key) {
        String fullKey = formatKey(key);
        // GETDEL is supported in Redis 6.2+. If not supported, we might need a Lua script or just delete.
        // Spring Data Redis 'getAndDelete' uses GETDEL if available.
        return redisTemplate.opsForValue().getAndDelete(fullKey);
    }

    @Override
    public Set<String> getKeys() {
        Set<String> keys = new HashSet<>();
        String scanPattern = (prefix != null && !prefix.isEmpty()) ? prefix + ":*" : "*";

        ScanOptions options = ScanOptions.scanOptions().match(scanPattern).count(100).build();

        try (Cursor<String> cursor = redisTemplate.scan(options)) {
            while (cursor.hasNext()) {
                String key = cursor.next();
                if (prefix != null && !prefix.isEmpty()) {
                    if (key.startsWith(prefix + ":")) {
                        keys.add(key.substring(prefix.length() + 1));
                    } else {
                        keys.add(key);
                    }
                } else {
                    keys.add(key);
                }
            }
        }
        return keys;
    }

    @Override
    public boolean containsKey(String key) {
        return Boolean.TRUE.equals(redisTemplate.hasKey(formatKey(key)));
    }

    @Override
    public void clear() {
        String scanPattern = (prefix != null && !prefix.isEmpty()) ? prefix + ":*" : "*";
        ScanOptions options = ScanOptions.scanOptions().match(scanPattern).count(100).build();

        Set<String> keysToDelete = new HashSet<>();
        try (Cursor<String> cursor = redisTemplate.scan(options)) {
            while (cursor.hasNext()) {
                keysToDelete.add(cursor.next());
            }
        }

        if (!keysToDelete.isEmpty()) {
            redisTemplate.delete(keysToDelete);
        }
    }

    @Override
    public void checkHealth() throws Exception {
        try (RedisConnection connection = connectionFactory.getConnection()) {
            String response = connection.ping();
            if (!"PONG".equalsIgnoreCase(response)) {
                 throw new RuntimeException("Redis PING failed for cache " + name + ": " + response);
            }
        }
    }

    @Override
    public void close() {
        if (connectionFactory != null) {
            connectionFactory.destroy();
            logger.info("Closed Redis connection for cache '{}'", name);
        }
    }
}
