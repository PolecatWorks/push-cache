package com.polecatworks.pushcache.config;

import jakarta.validation.Valid;
import jakarta.validation.constraints.NotNull;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.validation.annotation.Validated;

@ConfigurationProperties
@Validated
public class AppConfig {

    @Valid
    @NotNull
    private HamsConfig hams;

    @Valid
    @NotNull
    private RuntimeConfig runtime;

    @Valid
    @NotNull
    private WebServiceConfig webservice;

    @Valid
    @NotNull
    private KafkaConfig kafka;

    @Valid
    @NotNull
    private StartupCheckConfig startupChecks;

    public HamsConfig getHams() {
        return hams;
    }

    public void setHams(HamsConfig hams) {
        this.hams = hams;
    }

    public RuntimeConfig getRuntime() {
        return runtime;
    }

    public void setRuntime(RuntimeConfig runtime) {
        this.runtime = runtime;
    }

    public WebServiceConfig getWebservice() {
        return webservice;
    }

    public void setWebservice(WebServiceConfig webservice) {
        this.webservice = webservice;
    }

    public KafkaConfig getKafka() {
        return kafka;
    }

    public void setKafka(KafkaConfig kafka) {
        this.kafka = kafka;
    }

    public StartupCheckConfig getStartupChecks() {
        return startupChecks;
    }

    public void setStartupChecks(StartupCheckConfig startupChecks) {
        this.startupChecks = startupChecks;
    }
}
