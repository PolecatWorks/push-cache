package com.polecatworks.pushcache.cli;

import com.polecatworks.pushcache.PushCacheApplication;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.context.event.ApplicationEnvironmentPreparedEvent;
import org.springframework.context.ApplicationListener;
import org.springframework.core.env.ConfigurableEnvironment;
import org.springframework.core.env.PropertiesPropertySource;
import picocli.CommandLine.Command;
import picocli.CommandLine.Option;
import java.io.File;
import java.util.Properties;

@Command(name = "push-cache", subcommands = { Cli.Version.class, Cli.Start.class, Cli.ConfigCheck.class })
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
            app.addListeners((ApplicationListener<ApplicationEnvironmentPreparedEvent>) event -> {
                ConfigurableEnvironment env = event.getEnvironment();
                String hamsAddress = env.getProperty("hams.address");
                if (hamsAddress != null) {
                    String[] parts = hamsAddress.split(":");
                    if (parts.length == 2) {
                        Properties props = new Properties();
                        props.put("management.server.address", parts[0]);
                        props.put("management.server.port", parts[1]);
                        props.put("management.endpoints.web.exposure.include", "health,prometheus");
                        env.getPropertySources().addFirst(new PropertiesPropertySource("hamsConfig", props));
                    }
                }
            });
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
}
