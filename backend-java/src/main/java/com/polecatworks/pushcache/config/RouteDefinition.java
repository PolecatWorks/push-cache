package com.polecatworks.pushcache.config;

import jakarta.validation.constraints.NotBlank;

public class RouteDefinition {

    @NotBlank
    private String path;

    @NotBlank
    private String store;

    public String getPath() {
        return path;
    }

    public void setPath(String path) {
        this.path = path;
    }

    public String getStore() {
        return store;
    }

    public void setStore(String store) {
        this.store = store;
    }
}
