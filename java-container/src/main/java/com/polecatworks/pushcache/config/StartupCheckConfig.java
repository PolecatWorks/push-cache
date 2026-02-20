package com.polecatworks.pushcache.config;

import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import java.time.Duration;

public class StartupCheckConfig {

    @Min(0)
    private int fails;

    @NotNull
    private Duration timeout;

    private boolean enabled;

    public int getFails() {
        return fails;
    }

    public void setFails(int fails) {
        this.fails = fails;
    }

    public Duration getTimeout() {
        return timeout;
    }

    public void setTimeout(Duration timeout) {
        this.timeout = timeout;
    }

    public boolean isEnabled() {
        return enabled;
    }

    public void setEnabled(boolean enabled) {
        this.enabled = enabled;
    }
}
