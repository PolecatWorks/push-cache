package com.polecatworks.pushcache.config;

import com.polecatworks.pushcache.web.RecordHandler;
import org.springframework.boot.web.server.WebServerFactoryCustomizer;
import org.springframework.boot.web.servlet.server.ConfigurableServletWebServerFactory;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.servlet.function.RouterFunction;
import org.springframework.web.servlet.function.RouterFunctions;
import org.springframework.web.servlet.function.ServerResponse;

import java.net.URI;

@Configuration
public class WebConfig {

    @Bean
    public WebServerFactoryCustomizer<ConfigurableServletWebServerFactory> webServerFactoryCustomizer(AppConfig appConfig) {
        return factory -> {
            URI address = appConfig.getWebservice().getAddress();
            if (address != null && address.getPort() != -1) {
                factory.setPort(address.getPort());
            }
        };
    }

    @Bean
    public RouterFunction<ServerResponse> route(AppConfig appConfig, RecordHandler handler) {
        String path = "/";
        if (appConfig.getWebservice().getAddress() != null) {
            String uriPath = appConfig.getWebservice().getAddress().getPath();
            if (uriPath != null && !uriPath.isEmpty()) {
                path = uriPath;
            }
        }

        // Normalize path: ensure it does not end with / unless it is just /
        if (path.length() > 1 && path.endsWith("/")) {
            path = path.substring(0, path.length() - 1);
        }

        return RouterFunctions.route()
                .path(path, builder -> builder
                        .GET("/", handler::listRecords)
                        .GET("/{id}", handler::getRecord)
                        .DELETE("/{id}", handler::deleteRecord)
                        .POST("/{id}", handler::createRecord)
                )
                .build();
    }
}
