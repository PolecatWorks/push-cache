package com.polecatworks.pushcache.config;

import jakarta.validation.constraints.NotNull;
import java.net.URI;
import java.util.ArrayList;
import java.util.List;

public class WebServiceConfig {

    @NotNull
    private URI address;

    @NotNull
    private List<String> forwardingHeaders = new ArrayList<>();

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
}
