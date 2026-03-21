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
import reactor.core.publisher.Flux;
import reactor.core.publisher.Mono;
import reactor.core.scheduler.Schedulers;

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
    public Mono<Void> put(String key, byte[] value) {
        return Mono.fromRunnable(() -> {
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
        }).subscribeOn(Schedulers.boundedElastic()).then();
    }

    @Override
    public Mono<byte[]> get(String key) {
        return Mono.fromCallable(() -> {
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
        }).subscribeOn(Schedulers.boundedElastic());
    }

    @Override
    public Mono<byte[]> remove(String key) {
        return Mono.fromCallable(() -> {
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
        }).subscribeOn(Schedulers.boundedElastic());
    }

    @Override
    public Flux<String> getKeys() {
        return Flux.create(sink -> {
            try {
                for (Document doc : collection.find().projection(new Document("key", 1))) {
                    if (doc.containsKey("key") && doc.get("key") instanceof String) {
                        sink.next(doc.getString("key"));
                    }
                }
                sink.complete();
            } catch (Exception e) {
                logger.error("Mongo getKeys error: {}", e.getMessage(), e);
                sink.error(new RuntimeException("Mongo getKeys error", e));
            }
        }).cast(String.class).subscribeOn(Schedulers.boundedElastic());
    }

    @Override
    public Mono<Boolean> containsKey(String key) {
        return Mono.fromCallable(() -> {
            try {
                long count = collection.countDocuments(Filters.eq("key", key));
                return count > 0;
            } catch (Exception e) {
                logger.error("Mongo containsKey error for key {}: {}", key, e.getMessage(), e);
                throw new RuntimeException("Mongo containsKey error", e);
            }
        }).subscribeOn(Schedulers.boundedElastic());
    }

    @Override
    public Mono<Void> clear() {
        return Mono.fromRunnable(() -> {
            try {
                collection.deleteMany(new Document());
            } catch (Exception e) {
                logger.error("Mongo clear error: {}", e.getMessage(), e);
                throw new RuntimeException("Mongo clear error", e);
            }
        }).subscribeOn(Schedulers.boundedElastic()).then();
    }

    @Override
    public Mono<Void> checkHealth() {
        return Mono.fromRunnable(() -> {
            mongoClient.getDatabase("admin").runCommand(new Document("ping", 1));
        }).subscribeOn(Schedulers.boundedElastic()).then();
    }

    @Override
    public void close() {
        if (mongoClient != null) {
            mongoClient.close();
            logger.info("Closed Mongo client for store: {}", name);
        }
    }
}
