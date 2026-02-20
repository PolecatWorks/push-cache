package com.polecatworks.pushcache;

import com.polecatworks.pushcache.service.HamsService;
import com.polecatworks.pushcache.service.KafkaConsumerService;
import com.polecatworks.pushcache.service.StartupCheckService;
import org.springframework.boot.CommandLineRunner;
import org.springframework.stereotype.Component;

@Component
public class StartupCheckRunner implements CommandLineRunner {

    private final StartupCheckService startupCheckService;
    private final KafkaConsumerService kafkaConsumerService;
    private final HamsService hamsService;

    public StartupCheckRunner(StartupCheckService startupCheckService, KafkaConsumerService kafkaConsumerService, HamsService hamsService) {
        this.startupCheckService = startupCheckService;
        this.kafkaConsumerService = kafkaConsumerService;
        this.hamsService = hamsService;
    }

    @Override
    public void run(String... args) throws Exception {
        startupCheckService.runStartupChecks();
        hamsService.runPreflights();
        kafkaConsumerService.start();
    }
}
