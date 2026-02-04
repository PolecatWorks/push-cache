package com.polecatworks.pushcache;

import com.polecatworks.pushcache.config.AppConfig;
import org.springframework.boot.CommandLineRunner;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.boot.context.properties.EnableConfigurationProperties;
import org.springframework.context.annotation.Bean;
import com.fasterxml.jackson.databind.ObjectMapper;

@SpringBootApplication
@EnableConfigurationProperties(AppConfig.class)
public class PushCacheApplication {

	public static void main(String[] args) {
		SpringApplication.run(PushCacheApplication.class, args);
	}

    @Bean
    public CommandLineRunner printConfig(AppConfig appConfig, ObjectMapper objectMapper) {
        return args -> {
            System.out.println("Loaded Configuration:");
            // Ensure pretty printing is enabled for this write
            System.out.println(objectMapper.writerWithDefaultPrettyPrinter().writeValueAsString(appConfig));
        };
    }

}
