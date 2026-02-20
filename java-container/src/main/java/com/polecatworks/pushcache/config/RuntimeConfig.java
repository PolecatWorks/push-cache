package com.polecatworks.pushcache.config;

import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;

public class RuntimeConfig {

    @Min(0)
    private int threads;

    @Min(1)
    private int stackSize;

    @NotBlank
    private String name;

    public int getThreads() {
        return threads;
    }

    public void setThreads(int threads) {
        this.threads = threads;
    }

    public int getStackSize() {
        return stackSize;
    }

    public void setStackSize(int stackSize) {
        this.stackSize = stackSize;
    }

    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }
}
