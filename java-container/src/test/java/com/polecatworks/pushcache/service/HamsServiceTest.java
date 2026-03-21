package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.config.HamsConfig;
import io.micrometer.core.instrument.MeterRegistry;
import io.micrometer.prometheusmetrics.PrometheusMeterRegistry;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.boot.actuate.health.Health;
import org.springframework.boot.actuate.health.Status;
import org.springframework.http.HttpStatus;
import org.springframework.web.client.RestClient;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

class HamsServiceTest {

    private AppConfig appConfig;
    private LagClearedHealthIndicator lagHealthIndicator;
    private CacheHealthIndicator cacheHealthIndicator;
    private PrometheusMeterRegistry meterRegistry;
    private HamsService hamsService;
    private HttpClient httpClient;

    @BeforeEach
    void setUp() {
        appConfig = new AppConfig();
        HamsConfig hamsConfig = new HamsConfig();
        hamsConfig.setAddress("localhost:8079");
        hamsConfig.setPrefix("hams");
        appConfig.setHams(hamsConfig);

        lagHealthIndicator = mock(LagClearedHealthIndicator.class);
        cacheHealthIndicator = mock(CacheHealthIndicator.class);
        meterRegistry = mock(PrometheusMeterRegistry.class);

        // Reset before each test properly!
        when(lagHealthIndicator.health()).thenReturn(Health.up().build());
        when(cacheHealthIndicator.health()).thenReturn(Health.up().build());
        when(meterRegistry.scrape()).thenReturn("# HELP test\n# TYPE test gauge\ntest 1.0");

        hamsService = new HamsService(appConfig, lagHealthIndicator, cacheHealthIndicator, meterRegistry);
        hamsService.startServer();

        httpClient = HttpClient.newHttpClient();
    }

    @AfterEach
    void tearDown() {
        hamsService.destroy();
    }

    @Test
    void testAliveEndpoint() throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:8079/hams/alive"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(200, response.statusCode());
        assertEquals("OK", response.body());
    }

    @Test
    void testReadyEndpoint_Healthy() throws IOException, InterruptedException {
        when(lagHealthIndicator.health()).thenReturn(Health.up().build());
        when(cacheHealthIndicator.health()).thenReturn(Health.up().build());

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:8079/hams/ready"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(200, response.statusCode());
        assertEquals("Ready", response.body());
    }

    @Test
    void testReadyEndpoint_UnhealthyLag() throws IOException, InterruptedException {
        when(lagHealthIndicator.health()).thenReturn(Health.down().build());
        when(cacheHealthIndicator.health()).thenReturn(Health.up().build());

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:8079/hams/ready"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(503, response.statusCode());
        assertEquals("Service Unavailable", response.body());
    }

    @Test
    void testReadyEndpoint_UnhealthyCache() throws IOException, InterruptedException {
        when(lagHealthIndicator.health()).thenReturn(Health.up().build());
        when(cacheHealthIndicator.health()).thenReturn(Health.down().build());

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:8079/hams/ready"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(503, response.statusCode());
        assertEquals("Service Unavailable", response.body());
    }

    @Test
    void testStartupEndpoint_Healthy() throws IOException, InterruptedException {
        when(cacheHealthIndicator.health()).thenReturn(Health.up().build());

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:8079/hams/startup"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(200, response.statusCode());
        assertEquals("Startup OK", response.body());
    }

    @Test
    void testStartupEndpoint_UnhealthyCache() throws IOException, InterruptedException {
        when(cacheHealthIndicator.health()).thenReturn(Health.down().build());

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:8079/hams/startup"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(503, response.statusCode());
        assertEquals("Service Unavailable", response.body());
    }

    @Test
    void testMetricsEndpoint() throws IOException, InterruptedException {
        when(meterRegistry.scrape()).thenReturn("# HELP test\n# TYPE test gauge\ntest 1.0");

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://localhost:8079/hams/metrics"))
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(200, response.statusCode());
        assertTrue(response.body().contains("# HELP test"));
        assertEquals("text/plain; version=0.0.4; charset=utf-8", response.headers().firstValue("Content-Type").orElse(""));
    }
}
