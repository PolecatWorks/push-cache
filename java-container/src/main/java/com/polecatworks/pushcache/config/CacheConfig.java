package com.polecatworks.pushcache.config;

import jakarta.validation.Valid;
import jakarta.validation.constraints.NotNull;
import java.util.List;
import java.util.ArrayList;

public class CacheConfig {

    @Valid
    @NotNull
    private List<StoreDefinition> stores = new ArrayList<>();

    @Valid
    @NotNull
    private List<RouteDefinition> routes = new ArrayList<>();

    public List<StoreDefinition> getStores() {
        return stores;
    }

    public void setStores(List<StoreDefinition> stores) {
        this.stores = stores;
    }

    public List<RouteDefinition> getRoutes() {
        return routes;
    }

    public void setRoutes(List<RouteDefinition> routes) {
        this.routes = routes;
    }
}
