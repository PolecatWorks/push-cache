package com.polecatworks.pushcache.cli;

import com.polecatworks.pushcache.PushCacheApplication;
import com.polecatworks.pushcache.config.AppConfig;
import com.polecatworks.pushcache.config.StoreDefinition;
import com.polecatworks.pushcache.service.OracleCache;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.boot.SpringApplication;
import org.springframework.context.ApplicationContext;
import picocli.CommandLine.Command;
import picocli.CommandLine.Option;
import java.io.File;
import org.flywaydb.core.Flyway;

@Command(name = "push-cache", subcommands = { Cli.Version.class, Cli.Start.class, Cli.ConfigCheck.class, Cli.CreateSchemas.class })
public class Cli {

    @Command(name = "version", description = "Show version of application")
    public static class Version implements Runnable {
        @Override
        public void run() {
            System.out.println("push-cache Version: :0.0.1-SNAPSHOT");
        }
    }

    @Command(name = "start", description = "Start the http service")
    public static class Start implements Runnable {
        private static final Logger logger = LoggerFactory.getLogger(Start.class);

        @Option(names = { "-c", "--config" }, required = true, description = "Sets a custom config file")
        public File config;

        @Option(names = { "-s",
                "--secrets" }, defaultValue = "secrets", description = "Sets a custom secrets directory")
        public File secrets;

        @Override
        public void run() {
            logger.info("Starting push-cache:0.0.1-SNAPSHOT");
            System.setProperty("spring.config.additional-location", "file:" + config.getAbsolutePath());
            System.setProperty("spring.config.import", "optional:configtree:" + secrets.getAbsolutePath() + "/");

            SpringApplication app = new SpringApplication(PushCacheApplication.class);
            app.run();
        }
    }

    @Command(name = "config-check", description = "Check configuration")
    public static class ConfigCheck implements Runnable {
        private static final Logger logger = LoggerFactory.getLogger(ConfigCheck.class);

        @Option(names = { "-c", "--config" }, required = true, description = "Sets a custom config file")
        public File config;

        @Option(names = { "-s",
                "--secrets" }, defaultValue = "secrets", description = "Sets a custom secrets directory")
        public File secrets;

        @Override
        public void run() {
            logger.info("Config check push-cache for 0.0.1-SNAPSHOT");
            System.setProperty("spring.config.additional-location", "file:" + config.getAbsolutePath());
            System.setProperty("spring.config.import", "optional:configtree:" + secrets.getAbsolutePath() + "/");
            System.setProperty("spring.main.web-application-type", "none");
            System.setProperty("startup-checks.enabled", "false");
            SpringApplication.run(PushCacheApplication.class).close();
        }
    }

    @Command(name = "create-schemas", description = "Create required database schemas for caches")
    public static class CreateSchemas implements Runnable {
        private static final Logger logger = LoggerFactory.getLogger(CreateSchemas.class);

        @Option(names = { "-c", "--config" }, required = true, description = "Sets a custom config file")
        public File config;

        @Option(names = { "-s",
                "--secrets" }, defaultValue = "secrets", description = "Sets a custom secrets directory")
        public File secrets;

        @Override
        public void run() {
            logger.info("Creating schemas for push-cache 0.0.1-SNAPSHOT");
            System.setProperty("spring.config.additional-location", "file:" + config.getAbsolutePath());
            System.setProperty("spring.config.import", "optional:configtree:" + secrets.getAbsolutePath() + "/");
            System.setProperty("spring.main.web-application-type", "none");
            System.setProperty("startup-checks.enabled", "false");

            ApplicationContext ctx = SpringApplication.run(PushCacheApplication.class);
            AppConfig appConfig = ctx.getBean(AppConfig.class);

            if (appConfig.getCache() != null && appConfig.getCache().getStores() != null) {
                for (StoreDefinition storeDef : appConfig.getCache().getStores()) {
                    if (storeDef.getType() == StoreDefinition.StoreType.ORACLE) {
                        logger.info("Creating schema for Oracle store: {}", storeDef.getName());
                        try (OracleCache oracleCache = new OracleCache(storeDef)) {
                            Flyway flyway = Flyway.configure()
                                    .dataSource(oracleCache.getDataSource())
                                    .locations("classpath:db/migration")
                                    .placeholders(java.util.Collections.singletonMap("tableName", storeDef.getTableName()))
                                    .load();
                            flyway.migrate();
                            logger.info("Successfully created schema for store: {}", storeDef.getName());
                        } catch (Exception e) {
                            logger.error("Failed to create schema for Oracle store: {}", storeDef.getName(), e);
                            throw new RuntimeException("Schema creation failed", e);
                        }
                    }
                }
            }
            logger.info("Finished creating schemas.");
            ((org.springframework.context.ConfigurableApplicationContext) ctx).close();
        }
    }
}
