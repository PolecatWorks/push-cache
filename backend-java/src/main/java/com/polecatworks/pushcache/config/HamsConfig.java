package com.polecatworks.pushcache.config;

import jakarta.validation.Valid;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import java.util.ArrayList;
import java.util.List;

public class HamsConfig {

    @NotBlank
    private String address;

    @NotBlank
    private String prefix;

    private boolean logging;

    @NotNull
    @Valid
    private ChecksConfig checks;

    public String getAddress() {
        return address;
    }

    public void setAddress(String address) {
        this.address = address;
    }

    public String getPrefix() {
        return prefix;
    }

    public void setPrefix(String prefix) {
        this.prefix = prefix;
    }

    public boolean isLogging() {
        return logging;
    }

    public void setLogging(boolean logging) {
        this.logging = logging;
    }

    public ChecksConfig getChecks() {
        return checks;
    }

    public void setChecks(ChecksConfig checks) {
        this.checks = checks;
    }

    public static class ChecksConfig {
        @Min(0)
        private int timeout;

        @Min(0)
        private int fails;

        @NotNull
        private List<String> preflights = new ArrayList<>();

        @NotNull
        private List<String> shutdowns = new ArrayList<>();

        public int getTimeout() {
            return timeout;
        }

        public void setTimeout(int timeout) {
            this.timeout = timeout;
        }

        public int getFails() {
            return fails;
        }

        public void setFails(int fails) {
            this.fails = fails;
        }

        public List<String> getPreflights() {
            return preflights;
        }

        public void setPreflights(List<String> preflights) {
            this.preflights = preflights;
        }

        public List<String> getShutdowns() {
            return shutdowns;
        }

        public void setShutdowns(List<String> shutdowns) {
            this.shutdowns = shutdowns;
        }
    }
}
