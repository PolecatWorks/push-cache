package com.polecatworks.pushcache;

import com.polecatworks.pushcache.cli.Cli;
import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.service.Cache;
import com.polecatworks.pushcache.service.CacheFactory;
import org.springframework.boot.CommandLineRunner;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.boot.context.properties.EnableConfigurationProperties;
import org.springframework.context.annotation.Bean;
import com.fasterxml.jackson.databind.ObjectMapper;
import picocli.CommandLine;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

@SpringBootApplication
@EnableConfigurationProperties(AppConfig.class)
public class PushCacheApplication {
    private static final Logger logger = LoggerFactory.getLogger(PushCacheApplication.class);

    public static void main(String[] args) {
        if (args == null || args.length == 0) {
            String savedArgs = System.getProperty("app.saved.args");
            if (savedArgs != null) {
                args = savedArgs.split("\\|\\|\\|");
            }
        } else {
            System.setProperty("app.saved.args", String.join("|||", args));
        }

        CommandLine cmd = new CommandLine(new Cli());
        cmd.setExecutionExceptionHandler((ex, commandLine, parseResult) -> {
            if (ex.getClass().getName().endsWith("SilentExitException")) {
                return 0;
            }
            ex.printStackTrace();
            return commandLine.getCommandSpec().exitCodeOnExecutionException();
        });

        int exitCode = cmd.execute(args);
        if (exitCode != 0) {
            System.exit(exitCode);
        }
    }

    @Bean
    public CommandLineRunner printConfig(AppConfig appConfig, ObjectMapper objectMapper) {
        return args -> {
            if (logger.isDebugEnabled()) {
                String configStr = objectMapper.writerWithDefaultPrettyPrinter().writeValueAsString(appConfig);
                logger.debug("Loaded Configuration:\n{}", configStr);
            }
        };
    }

    @Bean
    public Cache defaultCache(CacheFactory cacheFactory) {
        return cacheFactory.getDefaultCache();
    }

}
