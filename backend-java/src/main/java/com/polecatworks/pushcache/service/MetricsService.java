package com.polecatworks.pushcache.service;

import io.micrometer.core.instrument.Counter;
import io.micrometer.core.instrument.Gauge;
import io.micrometer.core.instrument.MeterRegistry;
import org.springframework.stereotype.Service;

import java.util.concurrent.atomic.AtomicLong;

@Service
public class MetricsService {

    private final Counter requestsTotal;
    private final Counter requestsMiss;
    private final Counter updatesReceived;
    private final Counter tombstonesProcessed;
    private final Counter schemaMismatchCount;

    private final AtomicLong cacheSize = new AtomicLong(0);
    private final AtomicLong consumerLag = new AtomicLong(0);

    public MetricsService(MeterRegistry registry) {
        this.requestsTotal = Counter.builder("requests_total")
                .description("Total user info requests")
                .register(registry);
        this.requestsMiss = Counter.builder("requests_miss")
                .description("Total requests with no record found")
                .register(registry);
        this.updatesReceived = Counter.builder("updates_received")
                .description("Total updates received from Kafka")
                .register(registry);
        this.tombstonesProcessed = Counter.builder("tombstones_processed")
                .description("Total tombstone records processed")
                .register(registry);
        this.schemaMismatchCount = Counter.builder("schema_mismatch_count")
                .description("Total messages with schema mismatch")
                .register(registry);

        Gauge.builder("push_cache_records_total", cacheSize, AtomicLong::get)
                .description("Total records in cache")
                .register(registry);

        Gauge.builder("push_cache_consumer_lag_total", consumerLag, AtomicLong::get)
                .description("Total Kafka consumer lag")
                .register(registry);
    }

    public void incrementRequestsTotal() {
        requestsTotal.increment();
    }

    public void incrementRequestsMiss() {
        requestsMiss.increment();
    }

    public void incrementUpdatesReceived() {
        updatesReceived.increment();
    }

    public void incrementTombstonesProcessed() {
        tombstonesProcessed.increment();
    }

    public void incrementSchemaMismatchCount() {
        schemaMismatchCount.increment();
    }

    public void incrementCacheSize() {
        cacheSize.incrementAndGet();
    }

    public void decrementCacheSize() {
        cacheSize.decrementAndGet();
    }

    public void setCacheSize(long size) {
        cacheSize.set(size);
    }

    public void setConsumerLag(long lag) {
        consumerLag.set(lag);
    }

    public long getConsumerLag() {
        return consumerLag.get();
    }
}
