package com.polecatworks.pushcache.service;

import com.mongodb.client.MongoClient;
import com.mongodb.client.MongoClients;
import com.mongodb.client.MongoCollection;
import com.mongodb.client.MongoDatabase;
import com.mongodb.client.model.Filters;
import com.mongodb.client.model.UpdateOptions;
import com.mongodb.client.model.Updates;
import com.polecatworks.pushcache.config.StoreDefinition;
import org.bson.Document;
import org.bson.conversions.Bson;
import org.bson.types.Binary;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.HashSet;
import java.util.Set;

public class MongoCache implements Cache, AutoCloseable {

    private static final Logger logger = LoggerFactory.getLogger(MongoCache.class);

    private final String name;
    private final MongoClient mongoClient;
    private final MongoCollection<Document> collection;

    public MongoCache(StoreDefinition storeDef) {
        this.name = storeDef.getName();

        if (storeDef.getUrl() == null) {
            throw new IllegalArgumentException("Mongo URL is required for store: " + name);
        }
        if (storeDef.getDatabase() == null) {
            throw new IllegalArgumentException("Mongo database is required for store: " + name);
        }
        if (storeDef.getCollection() == null) {
            throw new IllegalArgumentException("Mongo collection is required for store: " + name);
        }

        String connectionString = storeDef.getUrl().toString();
        // The Java driver doesn't automatically map minPoolSize and maxPoolSize from URI to connection options if not embedded,
        // but typically standard connection strings like `mongodb://user:pass@host:port/db?minPoolSize=1&maxPoolSize=10` work fine.
        // For our purpose, if they are explicitly provided in config, we can append them or build settings.
        // We'll use the connection string directly for simplicity.
        this.mongoClient = MongoClients.create(connectionString);

        MongoDatabase database = mongoClient.getDatabase(storeDef.getDatabase());
        this.collection = database.getCollection(storeDef.getCollection());
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public void put(String key, byte[] value) {
        try {
            Bson filter = Filters.eq("key", key);
            Bson update = Updates.combine(
                    Updates.set("key", key),
                    Updates.set("value", new Binary(value))
            );
            UpdateOptions options = new UpdateOptions().upsert(true);
            collection.updateOne(filter, update, options);
        } catch (Exception e) {
            logger.error("Mongo insert error for key {}: {}", key, e.getMessage(), e);
            throw new RuntimeException("Mongo insert error", e);
        }
    }

    @Override
    public byte[] get(String key) {
        try {
            Document doc = collection.find(Filters.eq("key", key)).first();
            if (doc != null && doc.get("value") instanceof Binary) {
                return ((Binary) doc.get("value")).getData();
            }
            return null;
        } catch (Exception e) {
            logger.error("Mongo get error for key {}: {}", key, e.getMessage(), e);
            throw new RuntimeException("Mongo get error", e);
        }
    }

    @Override
    public byte[] remove(String key) {
        try {
            Document doc = collection.findOneAndDelete(Filters.eq("key", key));
            if (doc != null && doc.get("value") instanceof Binary) {
                return ((Binary) doc.get("value")).getData();
            }
            return null;
        } catch (Exception e) {
            logger.error("Mongo remove error for key {}: {}", key, e.getMessage(), e);
            throw new RuntimeException("Mongo remove error", e);
        }
    }

    @Override
    public Set<String> getKeys() {
        try {
            Set<String> keys = new HashSet<>();
            for (Document doc : collection.find().projection(new Document("key", 1))) {
                if (doc.containsKey("key") && doc.get("key") instanceof String) {
                    keys.add(doc.getString("key"));
                }
            }
            return keys;
        } catch (Exception e) {
            logger.error("Mongo getKeys error: {}", e.getMessage(), e);
            throw new RuntimeException("Mongo getKeys error", e);
        }
    }

    @Override
    public boolean containsKey(String key) {
        try {
            long count = collection.countDocuments(Filters.eq("key", key));
            return count > 0;
        } catch (Exception e) {
            logger.error("Mongo containsKey error for key {}: {}", key, e.getMessage(), e);
            throw new RuntimeException("Mongo containsKey error", e);
        }
    }

    @Override
    public void clear() {
        try {
            collection.deleteMany(new Document());
        } catch (Exception e) {
            logger.error("Mongo clear error: {}", e.getMessage(), e);
            throw new RuntimeException("Mongo clear error", e);
        }
    }

    @Override
    public void checkHealth() throws Exception {
        // Run a simple ping command to check connectivity
        mongoClient.getDatabase("admin").runCommand(new Document("ping", 1));
    }

    @Override
    public void close() {
        if (mongoClient != null) {
            mongoClient.close();
            logger.info("Closed Mongo client for store: {}", name);
        }
    }
}
