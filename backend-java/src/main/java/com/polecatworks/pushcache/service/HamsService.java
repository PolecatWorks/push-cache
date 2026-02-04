package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.config.HamsConfig;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.DisposableBean;
import org.springframework.stereotype.Service;
import org.springframework.web.client.RestClient;

@Service
public class HamsService implements DisposableBean {

    private static final Logger logger = LoggerFactory.getLogger(HamsService.class);
    private final AppConfig appConfig;
    private final RestClient restClient;

    public HamsService(AppConfig appConfig) {
        this.appConfig = appConfig;
        this.restClient = RestClient.builder().build();
    }

    public void runPreflights() {
        HamsConfig.ChecksConfig config = appConfig.getHams().getChecks();
        if (config == null) return;

        for (String url : config.getPreflights()) {
            runCheck("Preflight " + url, config.getFails(), config.getTimeout(), () -> {
                logger.info("Checking preflight: {}", url);
                restClient.get().uri(url).retrieve().toBodilessEntity();
            });
        }
    }

    public void runShutdowns() {
        HamsConfig.ChecksConfig config = appConfig.getHams().getChecks();
        if (config == null) return;

        for (String url : config.getShutdowns()) {
            runCheck("Shutdown " + url, config.getFails(), config.getTimeout(), () -> {
                logger.info("Checking shutdown: {}", url);
                restClient.get().uri(url).retrieve().toBodilessEntity();
            });
        }
    }

    @Override
    public void destroy() {
        runShutdowns();
    }

    private void runCheck(String name, int retries, int timeoutSec, Runnable check) {
        int attempts = retries;
        while (attempts > 0) {
            try {
                check.run();
                logger.info("Check passed: {}", name);
                return;
            } catch (Exception e) {
                 logger.warn("Check failed: {} retrying in {} secs (fail count {}/{})", name, timeoutSec, attempts, retries);
            }

            attempts--;
            if (attempts > 0) {
                try {
                    Thread.sleep(timeoutSec * 1000L);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new RuntimeException("Interrupted", e);
                }
            }
        }
        logger.error("{} FAIL", name);
        throw new RuntimeException(name + " Failed");
    }
}
