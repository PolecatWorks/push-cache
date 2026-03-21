package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.config.HamsConfig;
import com.sun.net.httpserver.HttpServer;
import io.micrometer.core.instrument.MeterRegistry;
import io.micrometer.prometheusmetrics.PrometheusMeterRegistry;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.DisposableBean;
import org.springframework.boot.actuate.health.Status;
import org.springframework.stereotype.Service;
import org.springframework.web.client.RestClient;

import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;

@Service
public class HamsService implements DisposableBean {

    private static final Logger logger = LoggerFactory.getLogger(HamsService.class);
    private final AppConfig appConfig;
    private final RestClient restClient;
    private final LagClearedHealthIndicator lagHealthIndicator;
    private final CacheHealthIndicator cacheHealthIndicator;
    private final MeterRegistry meterRegistry;
    private HttpServer server;

    public HamsService(AppConfig appConfig, LagClearedHealthIndicator lagHealthIndicator, CacheHealthIndicator cacheHealthIndicator, MeterRegistry meterRegistry) {
        this.appConfig = appConfig;
        this.restClient = RestClient.builder().build();
        this.lagHealthIndicator = lagHealthIndicator;
        this.cacheHealthIndicator = cacheHealthIndicator;
        this.meterRegistry = meterRegistry;
    }

    @PostConstruct
    public void startServer() {
        HamsConfig config = appConfig.getHams();
        String addressStr = config.getAddress();
        String prefix = config.getPrefix();

        if (addressStr == null || prefix == null) {
            logger.warn("Hams address or prefix not configured, skipping Hams server start");
            return;
        }

        try {
            String[] parts = addressStr.split(":");
            if (parts.length != 2) {
                logger.error("Invalid Hams address format: {}", addressStr);
                return;
            }
            String host = parts[0];
            int port = Integer.parseInt(parts[1]);

            server = HttpServer.create(new InetSocketAddress(host, port), 0);

            String basePath = "/" + prefix;
            // Ensure single leading slash and no trailing slash for clean concatenation
            if (!basePath.startsWith("/"))
                basePath = "/" + basePath;
            if (basePath.endsWith("/"))
                basePath = basePath.substring(0, basePath.length() - 1);

            // Handler for /alive
            server.createContext(basePath + "/alive", (exchange) -> {
                String response = "OK";
                exchange.sendResponseHeaders(200, response.length());
                try (OutputStream os = exchange.getResponseBody()) {
                    os.write(response.getBytes());
                }
            });

            // Handler for /ready
            server.createContext(basePath + "/ready", (exchange) -> {
                boolean isLagCleared = lagHealthIndicator.health().getStatus() == Status.UP;
                boolean isCacheUp = cacheHealthIndicator.health().getStatus() == Status.UP;

                if (isLagCleared && isCacheUp) {
                    String response = "Ready";
                    exchange.sendResponseHeaders(200, response.length());
                    try (OutputStream os = exchange.getResponseBody()) {
                        os.write(response.getBytes());
                    }
                } else {
                    String response = "Service Unavailable";
                    exchange.sendResponseHeaders(503, response.length());
                    try (OutputStream os = exchange.getResponseBody()) {
                        os.write(response.getBytes());
                    }
                }
            });

            // Handler for /startup
            server.createContext(basePath + "/startup", (exchange) -> {
                boolean isCacheUp = cacheHealthIndicator.health().getStatus() == Status.UP;

                if (isCacheUp) {
                    String response = "Startup OK";
                    exchange.sendResponseHeaders(200, response.length());
                    try (OutputStream os = exchange.getResponseBody()) {
                        os.write(response.getBytes());
                    }
                } else {
                    String response = "Service Unavailable";
                    exchange.sendResponseHeaders(503, response.length());
                    try (OutputStream os = exchange.getResponseBody()) {
                        os.write(response.getBytes());
                    }
                }
            });

            // Handler for /metrics
            logger.info("Registering metrics handler at {}", basePath + "/metrics");
            server.createContext(basePath + "/metrics", (exchange) -> {
                String response;
                if (meterRegistry instanceof PrometheusMeterRegistry) {
                    response = ((PrometheusMeterRegistry) meterRegistry).scrape();
                } else {
                    response = "# Metrics not available in prometheus format";
                }
                exchange.getResponseHeaders().set("Content-Type", "text/plain; version=0.0.4; charset=utf-8");
                exchange.sendResponseHeaders(200, response.getBytes().length);
                try (OutputStream os = exchange.getResponseBody()) {
                    os.write(response.getBytes());
                }
            });

            server.setExecutor(null); // creates a default executor
            server.start();
            logger.info("Hams server started at http://{}:{}{}", host, port, basePath);

        } catch (IOException e) {
            logger.error("Failed to start Hams server", e);
        } catch (NumberFormatException e) {
            logger.error("Invalid Hams port in address: {}", addressStr, e);
        }
    }

    public void runPreflights() {
        HamsConfig.ChecksConfig config = appConfig.getHams().getChecks();
        if (config == null)
            return;

        for (String url : config.getPreflights()) {
            runCheck("Preflight " + url, config.getFails(), config.getTimeout(), () -> {
                logger.info("Checking preflight: {}", url);
                restClient.get().uri(url).retrieve().toBodilessEntity();
            });
        }
    }

    public void runShutdowns() {
        HamsConfig.ChecksConfig config = appConfig.getHams().getChecks();
        if (config == null)
            return;

        for (String url : config.getShutdowns()) {
            runCheck("Shutdown " + url, config.getFails(), config.getTimeout(), () -> {
                logger.info("Checking shutdown: {}", url);
                restClient.get().uri(url).retrieve().toBodilessEntity();
            });
        }
    }

    @Override
    public void destroy() {
        if (server != null) {
            server.stop(0);
            logger.info("Hams server stopped");
        }
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
                logger.warn("Check failed: {} retrying in {} secs (fail count {}/{})", name, timeoutSec, attempts,
                        retries);
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
