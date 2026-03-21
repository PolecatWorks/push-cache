package com.polecatworks.pushcache.config;

import com.polecatworks.pushcache.service.Cache;
import com.polecatworks.pushcache.service.CacheFactory;
import com.polecatworks.pushcache.service.MetricsService;
import com.polecatworks.pushcache.service.SchemaService;
import com.polecatworks.pushcache.web.RecordHandler;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.boot.web.server.WebServerFactoryCustomizer;
import org.springframework.boot.web.reactive.server.ConfigurableReactiveWebServerFactory;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.reactive.function.server.RouterFunction;
import org.springframework.web.reactive.function.server.RouterFunctions;
import org.springframework.web.reactive.function.server.ServerResponse;

import java.net.URI;

@Configuration
public class WebConfig {

    private static final Logger logger = LoggerFactory.getLogger(WebConfig.class);

    @Bean
    public WebServerFactoryCustomizer<ConfigurableReactiveWebServerFactory> webServerFactoryCustomizer(
            AppConfig appConfig) {
        return factory -> {
            URI address = appConfig.getWebservice().getAddress();
            if (address != null) {
                if (address.getPort() != -1) {
                    factory.setPort(address.getPort());
                }
                if (address.getHost() != null) {
                    try {
                        factory.setAddress(java.net.InetAddress.getByName(address.getHost()));
                    } catch (java.net.UnknownHostException e) {
                        logger.error("Unknown host: {}", address.getHost(), e);
                    }
                }
            }
        };
    }

    @Bean
    public RouterFunction<ServerResponse> route(AppConfig appConfig, CacheFactory cacheFactory,
            SchemaService schemaService, MetricsService metricsService) {
        RouterFunctions.Builder routeBuilder = RouterFunctions.route();

        String basePath = appConfig.getWebservice().getAddress().getPath();
        if (basePath == null) {
            basePath = "";
        }

        // Ensure basePath does not end with slash unless it's just root (which is empty
        // string effectively for concat)
        if (basePath.endsWith("/")) {
            basePath = basePath.substring(0, basePath.length() - 1);
        }

        if (appConfig.getCache().getRoutes() != null) {
            for (RouteDefinition routeDef : appConfig.getCache().getRoutes()) {
                Cache store = cacheFactory.getStore(routeDef.getStore());
                if (store == null) {
                    logger.error("Store not found: {}", routeDef.getStore());
                    continue;
                }

                RecordHandler handler = new RecordHandler(store, appConfig, schemaService, metricsService,
                        routeDef.getKeyFromBody());

                String fullPath = (basePath + routeDef.getPath()).replace("//", "/");
                // Ensure fullPath starts with /
                if (!fullPath.startsWith("/")) {
                    fullPath = "/" + fullPath;
                }
                // Ensure no trailing slash for exact matching unless desired
                if (fullPath.length() > 1 && fullPath.endsWith("/")) {
                    fullPath = fullPath.substring(0, fullPath.length() - 1);
                }

                logger.info("Mounting route {} to store {}", fullPath, routeDef.getStore());

                routeBuilder.path(fullPath, builder -> builder
                        .GET("", handler::listRecords)
                        .GET("/{id}", handler::getRecord)
                        .DELETE("/{id}", handler::deleteRecord)
                        .POST("/{id}", handler::createRecord));

                if (routeDef.getKeyFromBody() != null && !routeDef.getKeyFromBody().isEmpty()) {
                    String rawBodyPath = basePath + routeDef.getPath() + "_by_body";
                    String bodyPath = rawBodyPath.replace("//", "/");
                    if (!bodyPath.startsWith("/")) {
                        bodyPath = "/" + bodyPath;
                    }
                    logger.info("Mounting body route {} to store {}", bodyPath, routeDef.getStore());
                    routeBuilder.GET(bodyPath, handler::getRecordByBody);
                }
            }
        }

        return routeBuilder.build();
    }
}
