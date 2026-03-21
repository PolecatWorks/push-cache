package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.config.HamsConfig;
import io.micrometer.prometheusmetrics.PrometheusMeterRegistry;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.boot.actuate.health.Health;

import java.io.IOException;
import java.net.ServerSocket;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class HamsServiceTest {

    private HamsService hamsService;
    private HttpClient httpClient;
    private int port;

    // Create actual dummy instances instead of Mockito mocks to avoid cross-thread mock state issues
    private volatile boolean lagIsUp = true;
    private volatile boolean cacheIsUp = true;
    private volatile String prometheusScrape = "# HELP test\n# TYPE test gauge\ntest 1.0";

    @BeforeEach
    void setUp() throws IOException, InterruptedException {
        // Find a random free port
        try (ServerSocket socket = new ServerSocket(0)) {
            port = socket.getLocalPort();
        }

        AppConfig appConfig = new AppConfig();
        HamsConfig hamsConfig = new HamsConfig();
        hamsConfig.setAddress("localhost:" + port);
        hamsConfig.setPrefix("hams");
        appConfig.setHams(hamsConfig);

        lagIsUp = true;
        cacheIsUp = true;

        LagClearedHealthIndicator lagHealthIndicator = new LagClearedHealthIndicator() {
            @Override
            public Health health() {
                return lagIsUp ? Health.up().build() : Health.down().build();
            }
        };

        // Needs to implement CacheHealthIndicator specifically (though since we override health() it's fine)
        CacheHealthIndicator cacheHealthIndicator = new CacheHealthIndicator(null) {
            @Override
            public Health health() {
                return cacheIsUp ? Health.up().build() : Health.down().build();
            }
        };

        PrometheusMeterRegistry meterRegistry = new PrometheusMeterRegistry(io.micrometer.prometheusmetrics.PrometheusConfig.DEFAULT) {
            @Override
            public String scrape() {
                return prometheusScrape;
            }
        };

        hamsService = new HamsService(appConfig, lagHealthIndicator, cacheHealthIndicator, meterRegistry);
        hamsService.startServer();

        httpClient = HttpClient.newHttpClient();

        // Give server a tiny bit of time to bind properly
        Thread.sleep(50);
    }

    @AfterEach
    void tearDown() {
        if (hamsService != null) {
            hamsService.destroy();
        }
    }

    @Test
    void testAliveEndpoint() throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:" + port + "/hams/alive"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(200, response.statusCode());
        assertEquals("OK", response.body());
    }

    @Test
    void testReadyEndpoint_Healthy() throws IOException, InterruptedException {
        lagIsUp = true;
        cacheIsUp = true;

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:" + port + "/hams/ready"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(200, response.statusCode());
        assertEquals("Ready", response.body());
    }

    @Test
    void testReadyEndpoint_UnhealthyLag() throws IOException, InterruptedException {
        lagIsUp = false;
        cacheIsUp = true;

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:" + port + "/hams/ready"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(503, response.statusCode());
        assertEquals("Service Unavailable", response.body());
    }

    @Test
    void testReadyEndpoint_UnhealthyCache() throws IOException, InterruptedException {
        lagIsUp = true;
        cacheIsUp = false;

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:" + port + "/hams/ready"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(503, response.statusCode());
        assertEquals("Service Unavailable", response.body());
    }

    @Test
    void testStartupEndpoint_Healthy() throws IOException, InterruptedException {
        cacheIsUp = true;

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:" + port + "/hams/startup"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(200, response.statusCode());
        assertEquals("Startup OK", response.body());
    }

    @Test
    void testStartupEndpoint_UnhealthyCache() throws IOException, InterruptedException {
        cacheIsUp = false;

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:" + port + "/hams/startup"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(503, response.statusCode());
        assertEquals("Service Unavailable", response.body());
    }

    @Test
    void testMetricsEndpoint() throws IOException, InterruptedException {
        prometheusScrape = "# HELP test\n# TYPE test gauge\ntest 1.0";

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:" + port + "/hams/metrics"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(200, response.statusCode());
        assertTrue(response.body().contains("# HELP test"));
        assertEquals("text/plain; version=0.0.4; charset=utf-8", response.headers().firstValue("Content-Type").orElse(""));
    }
}
