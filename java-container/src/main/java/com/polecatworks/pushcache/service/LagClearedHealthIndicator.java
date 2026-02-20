package com.polecatworks.pushcache.service;

import org.springframework.boot.actuate.health.Health;
import org.springframework.boot.actuate.health.HealthIndicator;
import org.springframework.stereotype.Component;

@Component
public class LagClearedHealthIndicator implements HealthIndicator {
    private volatile boolean cleared = false;

    public void setCleared(boolean cleared) {
        this.cleared = cleared;
    }

    public boolean isCleared() {
        return cleared;
    }

    @Override
    public Health health() {
        if (cleared) {
            return Health.up().withDetail("lag-cleared", true).build();
        }
        return Health.down().withDetail("lag-cleared", false).build();
    }
}
