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
    public void put(String key, byte[] value) {
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
    }

    @Override
    public byte[] get(String key) {
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
    }

    @Override
    public byte[] remove(String key) {
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
    }

    @Override
    public Set<String> getKeys() {
        try {
            String sql = String.format("SELECT k FROM %s", tableName);
            List<String> keysList = jdbcTemplate.query(sql, (rs, rowNum) -> rs.getString("k"));
            return new HashSet<>(keysList);
        } catch (Exception e) {
            logger.error("Oracle getKeys error: {}", e.getMessage(), e);
            throw new RuntimeException("Oracle getKeys error", e);
        }
    }

    @Override
    public boolean containsKey(String key) {
        try {
            String sql = String.format("SELECT COUNT(*) FROM %s WHERE k = ?", tableName);
            Integer count = jdbcTemplate.queryForObject(sql, Integer.class, key);
            return count != null && count > 0;
        } catch (Exception e) {
            logger.error("Oracle containsKey error for key {}: {}", key, e.getMessage(), e);
            throw new RuntimeException("Oracle containsKey error", e);
        }
    }

    @Override
    public void clear() {
        try {
            String sql = String.format("DELETE FROM %s", tableName);
            jdbcTemplate.update(sql);
        } catch (Exception e) {
            logger.error("Oracle clear error: {}", e.getMessage(), e);
            throw new RuntimeException("Oracle clear error", e);
        }
    }

    @Override
    public void checkHealth() throws Exception {
        jdbcTemplate.execute("SELECT 1 FROM DUAL");
    }

    @Override
    public void close() {
        if (dataSource != null) {
            dataSource.close();
            logger.info("Closed Oracle data source for store: {}", name);
        }
    }
}
