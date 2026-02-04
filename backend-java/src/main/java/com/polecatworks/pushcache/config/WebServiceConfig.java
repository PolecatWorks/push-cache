package com.polecatworks.pushcache.config;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import java.net.URI;
import java.util.ArrayList;
import java.util.List;

public class WebServiceConfig {

    @NotNull
    private URI address;

    @NotNull
    private List<String> forwardingHeaders = new ArrayList<>();

    @NotBlank
    private String pathDynamic;

    public URI getAddress() {
        return address;
    }

    public void setAddress(URI address) {
        this.address = address;
    }

    public List<String> getForwardingHeaders() {
        return forwardingHeaders;
    }

    public void setForwardingHeaders(List<String> forwardingHeaders) {
        this.forwardingHeaders = forwardingHeaders;
    }

    public String getPathDynamic() {
        return pathDynamic;
    }

    public void setPathDynamic(String pathDynamic) {
        this.pathDynamic = pathDynamic;
    }
}
