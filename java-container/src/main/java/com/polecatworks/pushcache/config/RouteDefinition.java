package com.polecatworks.pushcache.config;

import jakarta.validation.constraints.NotBlank;

public class RouteDefinition {

    @NotBlank
    private String path;

    @NotBlank
    private String store;

    private String keyFromBody;

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

    public String getKeyFromBody() {
        return keyFromBody;
    }

    public void setKeyFromBody(String keyFromBody) {
        this.keyFromBody = keyFromBody;
    }
}
