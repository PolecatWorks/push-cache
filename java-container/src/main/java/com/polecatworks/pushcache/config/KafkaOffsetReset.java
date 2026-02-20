package com.polecatworks.pushcache.config;

public enum KafkaOffsetReset {
    EARLIEST,
    LATEST;

    @Override
    public String toString() {
        return name().toLowerCase();
    }
}
