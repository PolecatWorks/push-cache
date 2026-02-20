package com.polecatworks.pushcache.service;

import io.micrometer.core.instrument.Counter;
import io.micrometer.core.instrument.Gauge;
import io.micrometer.core.instrument.MeterRegistry;
import org.springframework.stereotype.Service;

import java.util.concurrent.atomic.AtomicLong;

@Service
public class MetricsService {

    private final MeterRegistry registry;
    private final Counter updatesReceived;
    private final Counter tombstonesProcessed;
    private final Counter schemaMismatchCount;
    private final Counter schemaUnroutedCount;

    private final AtomicLong consumerLag = new AtomicLong(0);

    public MetricsService(MeterRegistry registry) {
        this.registry = registry;
        this.updatesReceived = Counter.builder("updates_received")
                .description("Total updates received from Kafka")
                .register(registry);
        this.tombstonesProcessed = Counter.builder("tombstones_processed")
                .description("Total tombstone records processed")
                .register(registry);
        this.schemaMismatchCount = Counter.builder("schema_mismatch_count")
                .description("Total messages with schema mismatch")
                .register(registry);
        this.schemaUnroutedCount = Counter.builder("schema_unrouted_count")
                .description("Total messages where schema was not routed to any store")
                .register(registry);

        Gauge.builder("push_cache_consumer_lag_total", consumerLag, AtomicLong::get)
                .description("Total Kafka consumer lag")
                .register(registry);
    }

    public void incrementRequestsTotal(String storeName) {
        registry.counter("requests_total", "store_name", storeName).increment();
    }

    public void incrementRequestsMiss(String storeName) {
        registry.counter("requests_miss", "store_name", storeName).increment();
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

    public void incrementSchemaUnroutedCount() {
        schemaUnroutedCount.increment();
    }

    public void registerCacheSize(String storeName, AtomicLong size) {
        Gauge.builder("push_cache_records_total", size, AtomicLong::get)
                .description("Total records in cache")
                .tag("store_name", storeName)
                .register(registry);
    }

    public void setConsumerLag(long lag) {
        consumerLag.set(lag);
    }

    public long getConsumerLag() {
        return consumerLag.get();
    }
}
