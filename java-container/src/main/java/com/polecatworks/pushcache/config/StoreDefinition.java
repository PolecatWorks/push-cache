package com.polecatworks.pushcache.config;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;

import java.net.URI;
import java.util.List;

public class StoreDefinition {

    @NotBlank
    private String name;

    @NotNull
    private StoreType type;

    private List<String> schemas;

    // Redis and Mongo specific fields
    private URI url;
    private String prefix;

    // Mongo specific fields
    private String database;
    private String collection;

    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }

    public StoreType getType() {
        return type;
    }

    public void setType(StoreType type) {
        this.type = type;
    }

    public List<String> getSchemas() {
        return schemas;
    }

    public void setSchemas(List<String> schemas) {
        this.schemas = schemas;
    }

    public URI getUrl() {
        return url;
    }

    public void setUrl(URI url) {
        this.url = url;
    }

    public String getPrefix() {
        return prefix;
    }

    public void setPrefix(String prefix) {
        this.prefix = prefix;
    }

    public String getDatabase() {
        return database;
    }

    public void setDatabase(String database) {
        this.database = database;
    }

    public String getCollection() {
        return collection;
    }

    public void setCollection(String collection) {
        this.collection = collection;
    }

    public enum StoreType {
        @JsonProperty("in_memory")
        IN_MEMORY,
        @JsonProperty("redis")
        REDIS,
        @JsonProperty("mongo")
        MONGO
    }
}
