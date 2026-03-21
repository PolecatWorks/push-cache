package com.polecatworks.pushcache.service;

import com.polecatworks.pushcache.config.StoreDefinition;
import com.zaxxer.hikari.HikariConfig;
import com.zaxxer.hikari.HikariDataSource;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.jdbc.core.RowMapper;
import org.springframework.jdbc.datasource.DataSourceTransactionManager;
import org.springframework.transaction.support.TransactionTemplate;
import reactor.core.publisher.Flux;
import reactor.core.publisher.Mono;
import reactor.core.scheduler.Schedulers;

import java.net.URI;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

public class OracleCache implements Cache, AutoCloseable {

    private static final Logger logger = LoggerFactory.getLogger(OracleCache.class);

    private final String name;
    private final String tableName;
    private final HikariDataSource dataSource;
    private final JdbcTemplate jdbcTemplate;
    private final TransactionTemplate transactionTemplate;

    public OracleCache(StoreDefinition storeDef) {
        this.name = storeDef.getName();

        if (storeDef.getUrl() == null) {
            throw new IllegalArgumentException("Oracle URL is required for store: " + name);
        }
        if (storeDef.getTableName() == null) {
            throw new IllegalArgumentException("Oracle tableName is required for store: " + name);
        }

        this.tableName = storeDef.getTableName();
        URI uri = storeDef.getUrl();

        HikariConfig config = new HikariConfig();

        // Formulate jdbc connection url
        // e.g. oracle://host:port/service_name
        String host = uri.getHost() != null ? uri.getHost() : "localhost";
        int port = uri.getPort() != -1 ? uri.getPort() : 1521;
        String path = uri.getPath(); // Should be /service_name
        if (path != null && path.startsWith("/")) {
            path = path.substring(1);
        }

        String jdbcUrl = String.format("jdbc:oracle:thin:@//%s:%d/%s", host, port, path);
        config.setJdbcUrl(jdbcUrl);
        config.setDriverClassName("oracle.jdbc.OracleDriver");

        // Extract user and password from URL if available
        String userInfo = uri.getUserInfo();
        if (userInfo != null) {
            String[] parts = userInfo.split(":");
            if (parts.length > 0) {
                config.setUsername(parts[0]);
            }
            if (parts.length > 1) {
                config.setPassword(parts[1]);
            }
        }

        config.setMinimumIdle(1);
        config.setMaximumPoolSize(10);

        this.dataSource = new HikariDataSource(config);
        this.jdbcTemplate = new JdbcTemplate(this.dataSource);

        DataSourceTransactionManager transactionManager = new DataSourceTransactionManager(this.dataSource);
        this.transactionTemplate = new TransactionTemplate(transactionManager);
    }

    public HikariDataSource getDataSource() {
        return dataSource;
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public Mono<Void> put(String key, byte[] value) {
        return Mono.fromRunnable(() -> {
            try {
                String sql = String.format(
                        "MERGE INTO %s t " +
                        "USING (SELECT ? AS k, CAST(? AS BLOB) AS v FROM dual) s " +
                        "ON (t.k = s.k) " +
                        "WHEN MATCHED THEN UPDATE SET t.v = s.v " +
                        "WHEN NOT MATCHED THEN INSERT (k, v) VALUES (s.k, s.v)",
                        tableName
                );
                jdbcTemplate.update(sql, key, value);
            } catch (Exception e) {
                logger.error("Oracle insert error for key {}: {}", key, e.getMessage(), e);
                throw new RuntimeException("Oracle insert error", e);
            }
        }).subscribeOn(Schedulers.boundedElastic()).then();
    }

    @Override
    public Mono<byte[]> get(String key) {
        return Mono.fromCallable(() -> {
            try {
                String sql = String.format("SELECT v FROM %s WHERE k = ?", tableName);
                List<byte[]> results = jdbcTemplate.query(sql, (rs, rowNum) -> rs.getBytes("v"), key);
                if (!results.isEmpty()) {
                    return results.get(0);
                }
                return null;
            } catch (Exception e) {
                logger.error("Oracle get error for key {}: {}", key, e.getMessage(), e);
                throw new RuntimeException("Oracle get error", e);
            }
        }).subscribeOn(Schedulers.boundedElastic());
    }

    @Override
    public Mono<byte[]> remove(String key) {
        return Mono.fromCallable(() -> {
            return transactionTemplate.execute(status -> {
                try {
                    // Fetch the old value first
                    String fetchSql = String.format("SELECT v FROM %s WHERE k = ?", tableName);
                    List<byte[]> results = jdbcTemplate.query(fetchSql, (rs, rowNum) -> rs.getBytes("v"), key);

                    byte[] oldValue = null;
                    if (!results.isEmpty()) {
                        oldValue = results.get(0);
                    }

                    // Now delete it
                    String deleteSql = String.format("DELETE FROM %s WHERE k = ?", tableName);
                    jdbcTemplate.update(deleteSql, key);

                    return oldValue;
                } catch (Exception e) {
                    status.setRollbackOnly();
                    logger.error("Oracle remove error for key {}: {}", key, e.getMessage(), e);
                    throw new RuntimeException("Oracle remove error", e);
                }
            });
        }).subscribeOn(Schedulers.boundedElastic());
    }

    @Override
    public Flux<String> getKeys() {
        return Mono.fromCallable(() -> {
            try {
                String sql = String.format("SELECT k FROM %s", tableName);
                return jdbcTemplate.query(sql, (rs, rowNum) -> rs.getString("k"));
            } catch (Exception e) {
                logger.error("Oracle getKeys error: {}", e.getMessage(), e);
                throw new RuntimeException("Oracle getKeys error", e);
            }
        }).subscribeOn(Schedulers.boundedElastic()).flatMapMany(Flux::fromIterable);
    }

    @Override
    public Mono<Boolean> containsKey(String key) {
        return Mono.fromCallable(() -> {
            try {
                String sql = String.format("SELECT COUNT(*) FROM %s WHERE k = ?", tableName);
                Integer count = jdbcTemplate.queryForObject(sql, Integer.class, key);
                return count != null && count > 0;
            } catch (Exception e) {
                logger.error("Oracle containsKey error for key {}: {}", key, e.getMessage(), e);
                throw new RuntimeException("Oracle containsKey error", e);
            }
        }).subscribeOn(Schedulers.boundedElastic());
    }

    @Override
    public Mono<Void> clear() {
        return Mono.fromRunnable(() -> {
            try {
                String sql = String.format("DELETE FROM %s", tableName);
                jdbcTemplate.update(sql);
            } catch (Exception e) {
                logger.error("Oracle clear error: {}", e.getMessage(), e);
                throw new RuntimeException("Oracle clear error", e);
            }
        }).subscribeOn(Schedulers.boundedElastic()).then();
    }

    @Override
    public Mono<Void> checkHealth() {
        return Mono.fromRunnable(() -> {
            jdbcTemplate.execute("SELECT 1 FROM DUAL");
        }).subscribeOn(Schedulers.boundedElastic()).then();
    }

    @Override
    public void close() {
        if (dataSource != null) {
            dataSource.close();
            logger.info("Closed Oracle data source for store: {}", name);
        }
    }
}
