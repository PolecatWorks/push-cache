package com.polecatworks.pushcache;

import com.polecatworks.pushcache.service.StartupCheckService;
import org.springframework.boot.CommandLineRunner;
import org.springframework.stereotype.Component;

@Component
public class StartupCheckRunner implements CommandLineRunner {

    private final StartupCheckService startupCheckService;

    public StartupCheckRunner(StartupCheckService startupCheckService) {
        this.startupCheckService = startupCheckService;
    }

    @Override
    public void run(String... args) throws Exception {
        startupCheckService.runStartupChecks();
    }
}
