package com.polecatworks.pushcache.config;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import java.net.URI;
import java.time.Duration;

public class KafkaConfig {

    @NotNull
    private URI brokers;

    @NotBlank
    private String groupId;

    @NotBlank
    private String topic;

    @NotNull
    private URI schemaRegistryUrl;

    @NotNull
    private Duration cacheMaxAge;

    @NotNull
    private Duration fetchMetadataTimeout;

    @NotNull
    private KafkaOffsetReset offsetReset;

    private boolean forceResetEarliest;

    public URI getBrokers() {
        return brokers;
    }

    public void setBrokers(URI brokers) {
        this.brokers = brokers;
    }

    public String getGroupId() {
        return groupId;
    }

    public void setGroupId(String groupId) {
        this.groupId = groupId;
    }

    public String getTopic() {
        return topic;
    }

    public void setTopic(String topic) {
        this.topic = topic;
    }

    public URI getSchemaRegistryUrl() {
        return schemaRegistryUrl;
    }

    public void setSchemaRegistryUrl(URI schemaRegistryUrl) {
        this.schemaRegistryUrl = schemaRegistryUrl;
    }

    public Duration getCacheMaxAge() {
        return cacheMaxAge;
    }

    public void setCacheMaxAge(Duration cacheMaxAge) {
        this.cacheMaxAge = cacheMaxAge;
    }

    public Duration getFetchMetadataTimeout() {
        return fetchMetadataTimeout;
    }

    public void setFetchMetadataTimeout(Duration fetchMetadataTimeout) {
        this.fetchMetadataTimeout = fetchMetadataTimeout;
    }

    public KafkaOffsetReset getOffsetReset() {
        return offsetReset;
    }

    public void setOffsetReset(KafkaOffsetReset offsetReset) {
        this.offsetReset = offsetReset;
    }

    public boolean isForceResetEarliest() {
        return forceResetEarliest;
    }

    public void setForceResetEarliest(boolean forceResetEarliest) {
        this.forceResetEarliest = forceResetEarliest;
    }
}
