.PHONY: backend-dev backend-test backend-app-auth frontend-dev \
	start-zookeeper start-kafka start-schema \
	topics-list topics-create topics-delete topic-describe \
	groups-list groups-describe

BE_DIR := backend
FE_DIR := frontend
IMAGE_NAME := push-cache

################################################################################
# Backend & Frontend
################################################################################

status-ports:
	@lsof -i tcp:8080
	@lsof -i tcp:4200


backend-dev: export CAPTURE_LOG=INFO
backend-dev:
	cd ${BE_DIR} && cargo watch  -x "run -- start --config test-data/config-localhost.yaml --secrets test-data/secrets"

backend-test:
	cd ${BE_DIR} && cargo watch --ignore test_data -x "test"

backend-docker: PKG_NAME=push-cache
backend-docker:
	{ \
	docker buildx build ${BE_DIR} -t $(IMAGE_NAME)-backend -f ${BE_DIR}/Dockerfile; \
	docker image ls $(IMAGE_NAME)-backend; \
	}

backend-docker-run: backend-docker
	docker run -it --rm --name $(IMAGE_NAME)-backend -p 8080:8080 --mount type=bind,src=$(PWD)/${BE_DIR}/test-data,dst=/test-data  \
	-e CAPTURE_LOG=INFO \
	$(IMAGE_NAME)-backend start --config /test-data/config-localhost.yaml --secrets /test-data/secrets


frontend-dev:
	cd frontend && ng serve

frontend-docker: PKG_NAME=push-cache
frontend-docker:
	{ \
	docker build ${FE_DIR} -t $(IMAGE_NAME)-frontend -f ${FE_DIR}/Dockerfile --build-arg PKG_NAME=${PKG_NAME}; \
	docker image ls $(IMAGE_NAME)-frontend; \
	}

frontend-docker-run: frontend-docker
	docker run -it --rm -p 4201:8080 --name $(IMAGE_NAME)-frontend $(IMAGE_NAME)-frontend

################################################################################
# Kafka Infrastructure
################################################################################

KAFKA_BOOTSTRAP := localhost:9092
export CONFLUENT_HOME := $(HOME)/Development/kafka/confluent-7.3.1

SCHEMA_REGISTRY_START  := $(CONFLUENT_HOME)/bin/schema-registry-start
ZOOKEEPER_SERVER_START := $(CONFLUENT_HOME)/bin/zookeeper-server-start
KAFKA_SERVER_START     := $(CONFLUENT_HOME)/bin/kafka-server-start
KAFKA_TOPICS           := $(CONFLUENT_HOME)/bin/kafka-topics
KAFKA_PRODUCER         := $(CONFLUENT_HOME)/bin/kafka-console-producer
KAFKA_CONSUMER         := $(CONFLUENT_HOME)/bin/kafka-console-consumer
KAFKA_CONSUMER_GROUPS  := $(CONFLUENT_HOME)/bin/kafka-consumer-groups

start-zookeeper:
	$(ZOOKEEPER_SERVER_START) $(CONFLUENT_HOME)/etc/kafka/zookeeper.properties

start-kafka:
	$(KAFKA_SERVER_START) $(CONFLUENT_HOME)/etc/kafka/server.properties

start-schema:
	$(SCHEMA_REGISTRY_START) $(CONFLUENT_HOME)/etc/schema-registry/schema-registry.properties

################################################################################
# Kafka Utilities
################################################################################

topics-list:
	$(KAFKA_TOPICS) --bootstrap-server $(KAFKA_BOOTSTRAP) --list

topics-create:
	$(KAFKA_TOPICS) --bootstrap-server $(KAFKA_BOOTSTRAP) --create --topic "test.topic" --partitions 1 --replication-factor 1
	$(KAFKA_TOPICS) --bootstrap-server $(KAFKA_BOOTSTRAP) --create --topic "input" --partitions 1 --replication-factor 1
	$(KAFKA_TOPICS) --bootstrap-server $(KAFKA_BOOTSTRAP) --create --topic "output" --partitions 1 --replication-factor 1
	$(KAFKA_TOPICS) --bootstrap-server $(KAFKA_BOOTSTRAP) --create --topic "pcache-users" --partitions 1 --replication-factor 1

topics-delete:
	$(KAFKA_TOPICS) --bootstrap-server $(KAFKA_BOOTSTRAP) --delete --topic "test.topic"
	$(KAFKA_TOPICS) --bootstrap-server $(KAFKA_BOOTSTRAP) --delete --topic "input"
	$(KAFKA_TOPICS) --bootstrap-server $(KAFKA_BOOTSTRAP) --delete --topic "output"
	$(KAFKA_TOPICS) --bootstrap-server $(KAFKA_BOOTSTRAP) --delete --topic "pcache-users"

topic-describe:
	@if [ -z "$(TOPIC)" ]; then echo "Usage: make topic-describe TOPIC=<topic_name>"; exit 1; fi
	$(KAFKA_TOPICS) --bootstrap-server $(KAFKA_BOOTSTRAP) --describe --topic $(TOPIC)

groups-list:
	$(KAFKA_CONSUMER_GROUPS) --bootstrap-server $(KAFKA_BOOTSTRAP) --list

groups-describe:
	@if [ -z "$(GROUP)" ]; then echo "Usage: make groups-describe GROUP=<group_name>"; exit 1; fi
	$(KAFKA_CONSUMER_GROUPS) --bootstrap-server $(KAFKA_BOOTSTRAP) --describe --group $(GROUP)

topic-input-write:
	echo a:bcdef | $(KAFKA_PRODUCER) --topic input --bootstrap-server $(KAFKA_BOOTSTRAP) --property parse.key=true --property key.separator=":"

topic-input-read:
	@$(KAFKA_CONSUMER) --bootstrap-server $(KAFKA_BOOTSTRAP) --topic input --from-beginning --property print.key=true --property key.separator=":"
