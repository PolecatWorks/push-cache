package com.polecatworks.pushcache.service;

import org.springframework.boot.actuate.health.Health;
import org.springframework.boot.actuate.health.HealthIndicator;
import org.springframework.stereotype.Component;

import java.util.HashMap;
import java.util.Map;

@Component
public class CacheHealthIndicator implements HealthIndicator {

    private final CacheFactory cacheFactory;

    public CacheHealthIndicator(CacheFactory cacheFactory) {
        this.cacheFactory = cacheFactory;
    }

    @Override
    public Health health() {
        Map<String, String> details = new HashMap<>();
        boolean allHealthy = true;

        for (Cache cache : cacheFactory.getAllStores()) {
            try {
                cache.checkHealth().block();
                details.put(cache.getName(), "UP");
            } catch (Exception e) {
                allHealthy = false;
                details.put(cache.getName(), "DOWN: " + e.getMessage());
            }
        }

        if (allHealthy) {
            return Health.up().withDetails(details).build();
        } else {
            return Health.down().withDetails(details).build();
        }
    }
}
