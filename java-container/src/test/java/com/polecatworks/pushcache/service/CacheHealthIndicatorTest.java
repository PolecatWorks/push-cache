package com.polecatworks.pushcache.service;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.Mock;
import org.mockito.MockitoAnnotations;
import org.springframework.boot.actuate.health.Health;
import org.springframework.boot.actuate.health.Status;
import reactor.core.publisher.Mono;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.Mockito.*;

class CacheHealthIndicatorTest {

    @Mock
    private CacheFactory cacheFactory;

    private CacheHealthIndicator indicator;

    @BeforeEach
    void setUp() {
        MockitoAnnotations.openMocks(this);
        indicator = new CacheHealthIndicator(cacheFactory);
    }

    @Test
    void testHealthUp() throws Exception {
        Cache cache1 = mock(Cache.class);
        when(cache1.getName()).thenReturn("cache1");
        when(cache1.checkHealth()).thenReturn(Mono.empty());

        Cache cache2 = mock(Cache.class);
        when(cache2.getName()).thenReturn("cache2");
        when(cache2.checkHealth()).thenReturn(Mono.empty());

        when(cacheFactory.getAllStores()).thenReturn(List.of(cache1, cache2));

        Health health = indicator.health();

        assertEquals(Status.UP, health.getStatus());
        assertEquals("UP", health.getDetails().get("cache1"));
        assertEquals("UP", health.getDetails().get("cache2"));
    }

    @Test
    void testHealthDown() throws Exception {
        Cache cache1 = mock(Cache.class);
        when(cache1.getName()).thenReturn("cache1");
        when(cache1.checkHealth()).thenReturn(Mono.empty());

        Cache cache2 = mock(Cache.class);
        when(cache2.getName()).thenReturn("cache2");
        when(cache2.checkHealth()).thenReturn(Mono.error(new RuntimeException("Connection refused")));

        when(cacheFactory.getAllStores()).thenReturn(List.of(cache1, cache2));

        Health health = indicator.health();

        assertEquals(Status.DOWN, health.getStatus());
        assertEquals("UP", health.getDetails().get("cache1"));
        assertTrue(health.getDetails().get("cache2").toString().contains("DOWN"));
        assertTrue(health.getDetails().get("cache2").toString().contains("Connection refused"));
    }
}
