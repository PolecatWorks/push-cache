package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.config.StoreDefinition;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.DisposableBean;
import org.springframework.stereotype.Service;

import java.util.Collection;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;

@Service
public class CacheFactory implements DisposableBean {
    private static final Logger logger = LoggerFactory.getLogger(CacheFactory.class);

    private final Map<String, Cache> stores = new LinkedHashMap<>();
    private final Map<String, String> schemaToStore = new HashMap<>();
    private final Map<String, String> pathToStore = new HashMap<>();

    public CacheFactory(AppConfig appConfig, MetricsService metricsService) {
        // Initialize Stores
        if (appConfig.getCache().getStores() != null) {
            for (StoreDefinition storeDef : appConfig.getCache().getStores()) {
                if (storeDef.getType() == StoreDefinition.StoreType.IN_MEMORY) {
                    stores.put(storeDef.getName(), new InMemoryCache(storeDef.getName(), metricsService));
                    logger.info("Initialized InMemory store: {}", storeDef.getName());
                } else if (storeDef.getType() == StoreDefinition.StoreType.REDIS) {
                    try {
                        stores.put(storeDef.getName(), new RedisCache(storeDef));
                        logger.info("Initialized Redis store: {}", storeDef.getName());
                    } catch (Exception e) {
                        logger.error("Failed to initialize Redis store: {}", storeDef.getName(), e);
                        throw new RuntimeException("Failed to initialize Redis store: " + storeDef.getName(), e);
                    }
                } else if (storeDef.getType() == StoreDefinition.StoreType.MONGO) {
                    try {
                        stores.put(storeDef.getName(), new MongoCache(storeDef));
                        logger.info("Initialized Mongo store: {}", storeDef.getName());
                    } catch (Exception e) {
                        logger.error("Failed to initialize Mongo store: {}", storeDef.getName(), e);
                        throw new RuntimeException("Failed to initialize Mongo store: " + storeDef.getName(), e);
                    }
                } else if (storeDef.getType() == StoreDefinition.StoreType.ORACLE) {
                    try {
                        stores.put(storeDef.getName(), new OracleCache(storeDef));
                        logger.info("Initialized Oracle store: {}", storeDef.getName());
                    } catch (Exception e) {
                        logger.error("Failed to initialize Oracle store: {}", storeDef.getName(), e);
                        throw new RuntimeException("Failed to initialize Oracle store: " + storeDef.getName(), e);
                    }
                }

                // Map Schemas to Store
                if (storeDef.getSchemas() != null) {
                    for (String schema : storeDef.getSchemas()) {
                        schemaToStore.put(schema, storeDef.getName());
                    }
                }
            }
        }

        // Initialize Routes
        if (appConfig.getCache().getRoutes() != null) {
            appConfig.getCache().getRoutes().forEach(route -> {
                pathToStore.put(route.getPath(), route.getStore());
                logger.info("Mapped path '{}' to store '{}'", route.getPath(), route.getStore());
            });
        }
    }

    public Cache getStore(String name) {
        return stores.get(name);
    }

    public Collection<Cache> getAllStores() {
        return stores.values();
    }

    public Cache getStoreForSchema(String schemaName) {
        String storeName = schemaToStore.get(schemaName);
        if (storeName != null) {
            return stores.get(storeName);
        }
        return null;
    }

    public Cache getStoreForPath(String path) {
        String storeName = pathToStore.get(path);
        if (storeName != null) {
            return stores.get(storeName);
        }
        return null;
    }

    public Cache getDefaultCache() {
        if (stores.isEmpty()) {
            throw new IllegalStateException("No cache stores configured");
        }
        if (stores.containsKey("default")) {
            return stores.get("default");
        }
        return stores.values().iterator().next();
    }

    @Override
    public void destroy() {
        for (Cache cache : stores.values()) {
            if (cache instanceof AutoCloseable) {
                try {
                    ((AutoCloseable) cache).close();
                } catch (Exception e) {
                    logger.error("Error closing cache: {}", cache.getName(), e);
                }
            }
        }
    }
}
