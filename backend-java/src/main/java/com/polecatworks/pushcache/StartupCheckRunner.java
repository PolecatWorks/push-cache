package com.polecatworks.pushcache;

import com.polecatworks.pushcache.service.KafkaConsumerService;
import com.polecatworks.pushcache.service.StartupCheckService;
import org.springframework.boot.CommandLineRunner;
import org.springframework.stereotype.Component;

@Component
public class StartupCheckRunner implements CommandLineRunner {

    private final StartupCheckService startupCheckService;
    private final KafkaConsumerService kafkaConsumerService;

    public StartupCheckRunner(StartupCheckService startupCheckService, KafkaConsumerService kafkaConsumerService) {
        this.startupCheckService = startupCheckService;
        this.kafkaConsumerService = kafkaConsumerService;
    }

    @Override
    public void run(String... args) throws Exception {
        startupCheckService.runStartupChecks();
        kafkaConsumerService.start();
    }
}
