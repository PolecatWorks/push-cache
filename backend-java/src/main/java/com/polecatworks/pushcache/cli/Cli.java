package com.polecatworks.pushcache.cli;

import com.polecatworks.pushcache.PushCacheApplication;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.boot.SpringApplication;
import picocli.CommandLine.Command;
import picocli.CommandLine.Option;
import java.io.File;

@Command(name = "push-cache", subcommands = {Cli.Version.class, Cli.Start.class, Cli.ConfigCheck.class})
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

        @Option(names = {"-c", "--config"}, required = true, description = "Sets a custom config file")
        public File config;

        @Option(names = {"-s", "--secrets"}, defaultValue = "secrets", description = "Sets a custom secrets directory")
        public File secrets;

        @Override
        public void run() {
             logger.info("Starting push-cache:0.0.1-SNAPSHOT");
             System.setProperty("spring.config.additional-location", "file:" + config.getAbsolutePath());
             System.setProperty("spring.config.import", "optional:configtree:" + secrets.getAbsolutePath() + "/");
             SpringApplication.run(PushCacheApplication.class);
        }
    }

    @Command(name = "config-check", description = "Check configuration")
    public static class ConfigCheck implements Runnable {
        private static final Logger logger = LoggerFactory.getLogger(ConfigCheck.class);

        @Option(names = {"-c", "--config"}, required = true, description = "Sets a custom config file")
        public File config;

        @Option(names = {"-s", "--secrets"}, defaultValue = "secrets", description = "Sets a custom secrets directory")
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
}
