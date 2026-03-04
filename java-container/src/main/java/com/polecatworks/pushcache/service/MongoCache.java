package com.polecatworks.pushcache.service;

import com.mongodb.client.MongoClient;
import com.mongodb.client.MongoClients;
import com.mongodb.client.MongoCollection;
import com.mongodb.client.model.Filters;
import com.mongodb.client.model.UpdateOptions;
import com.mongodb.client.model.Updates;
import com.polecatworks.pushcache.config.StoreDefinition;
import org.bson.Document;
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

        try {
            // Note: The Mongo string requires the connection URI e.g. mongodb://user:pass@host:port/
            this.mongoClient = MongoClients.create(storeDef.getUrl().toString());
            this.collection = this.mongoClient.getDatabase(storeDef.getDatabase())
                    .getCollection(storeDef.getCollection());
        } catch (Exception e) {
            logger.error("Mongo connect error: {}", e.getMessage(), e);
            throw new RuntimeException("Mongo connect error: " + e.getMessage(), e);
        }
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public void put(String key, byte[] value) {
        try {
            Document filter = new Document("key", key);
            Binary binaryValue = new Binary(value);

            // Rust: let update = doc! { "$set": { "key": key, "value": bson::Binary { subtype: bson::spec::BinarySubtype::Generic, bytes: value } } };
            // Uses Upsert
            collection.updateOne(
                    filter,
                    Updates.combine(
                            Updates.set("key", key),
                            Updates.set("value", binaryValue)
                    ),
                    new UpdateOptions().upsert(true)
            );
        } catch (Exception e) {
            logger.error("Mongo update_one error: {}", e.getMessage(), e);
            throw new RuntimeException("Mongo insert error: " + e.getMessage(), e);
        }
    }

    @Override
    public byte[] get(String key) {
        try {
            Document doc = collection.find(Filters.eq("key", key)).first();
            if (doc != null && doc.get("value") instanceof Binary binaryValue) {
                return binaryValue.getData();
            }
            return null;
        } catch (Exception e) {
            logger.error("Mongo find_one error: {}", e.getMessage(), e);
            throw new RuntimeException("Mongo get error: " + e.getMessage(), e);
        }
    }

    @Override
    public byte[] remove(String key) {
        try {
            Document doc = collection.findOneAndDelete(Filters.eq("key", key));
            if (doc != null && doc.get("value") instanceof Binary binaryValue) {
                return binaryValue.getData();
            }
            return null;
        } catch (Exception e) {
            logger.error("Mongo find_one_and_delete error: {}", e.getMessage(), e);
            throw new RuntimeException("Mongo remove error: " + e.getMessage(), e);
        }
    }

    @Override
    public Set<String> getKeys() {
        try {
            Set<String> keys = new HashSet<>();
            for (Document doc : collection.find(new Document()).projection(new Document("key", 1))) {
                String key = doc.getString("key");
                if (key != null) {
                    keys.add(key);
                }
            }
            return keys;
        } catch (Exception e) {
            logger.error("Mongo find error: {}", e.getMessage(), e);
            throw new RuntimeException("Mongo keys error: " + e.getMessage(), e);
        }
    }

    @Override
    public boolean containsKey(String key) {
        try {
            long count = collection.countDocuments(Filters.eq("key", key));
            return count > 0;
        } catch (Exception e) {
            logger.error("Mongo count_documents error: {}", e.getMessage(), e);
            throw new RuntimeException("Mongo contains_key error: " + e.getMessage(), e);
        }
    }

    @Override
    public void clear() {
        try {
            collection.deleteMany(new Document());
        } catch (Exception e) {
            logger.error("Mongo deleteMany error: {}", e.getMessage(), e);
            throw new RuntimeException("Mongo clear error: " + e.getMessage(), e);
        }
    }

    @Override
    public void checkHealth() throws Exception {
        try {
            // A simple command to check connection
            mongoClient.getDatabase("admin").runCommand(new Document("ping", 1));
        } catch (Exception e) {
            logger.error("Mongo checkHealth error: {}", e.getMessage(), e);
            throw new Exception("Mongo checkHealth error: " + e.getMessage(), e);
        }
    }

    @Override
    public void close() throws Exception {
        if (mongoClient != null) {
            mongoClient.close();
        }
    }
}
